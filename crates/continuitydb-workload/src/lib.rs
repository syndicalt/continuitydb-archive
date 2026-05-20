//! Deterministic workload generation for ContinuityDB benchmarks.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{checkout, CheckoutError, CheckoutRequest};
use continuitydb_core::{
    ActivationState, Answerability, CellCost, CellDependency, CellDependencyKind, CellPayload,
    Citation, Confidence, Evidence, Scope, SemanticAnchor, StateCell, StateCellId, TrustSignal,
    UtilityFeedback, ValidTimeRange,
};
use continuitydb_kernel::{FileKernelLookupPlan, KernelError, StorageKernel};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use thiserror::Error;

/// Deterministic workload generation parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkloadConfig {
    /// Number of StateCells to generate.
    pub cell_count: usize,
    /// Numeric seed used as the base for deterministic StateCell IDs.
    pub id_seed: u128,
    /// Prefix for generated semantic anchors.
    pub anchor_prefix: String,
    /// Project scope for generated cells.
    pub project_scope: String,
    /// Valid-time start assigned to generated cells.
    pub valid_from: DateTime<Utc>,
    /// Every Nth cell becomes frontier.
    pub frontier_every: usize,
    /// Each cell after this stride depends on the cell at index minus stride.
    pub dependency_stride: usize,
}

/// Generated workload and summary metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuityWorkload {
    /// Generated StateCells in deterministic append order.
    pub cells: Vec<StateCell>,
    /// Summary of workload shape.
    pub summary: WorkloadSummary,
}

/// Deterministic workload shape summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadSummary {
    /// Number of generated StateCells.
    pub cell_count: usize,
    /// Number of generated cells marked as frontier.
    pub frontier_count: usize,
    /// Number of generated dependency edges.
    pub dependency_count: usize,
    /// Sum of generated token costs.
    pub total_token_cost: i64,
}

/// Measured operation count and elapsed wall-clock time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasuredOperation {
    /// Number of logical operations performed.
    pub operation_count: usize,
    /// Observed elapsed time for the operation group.
    pub elapsed: Duration,
}

/// Checkout result counts measured from a workload run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckoutMeasurement {
    /// Number of checkout candidates matching request constraints.
    pub matched_count: usize,
    /// Number of cells selected into the returned slice.
    pub selected_count: usize,
    /// Number of matching cells omitted as alternatives.
    pub alternative_count: usize,
    /// Number of selected frontier cells.
    pub frontier_count: usize,
    /// Selected token total reported by checkout.
    pub selected_token_count: i64,
}

/// End-to-end workload measurement over one kernel and checkout request.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkloadMeasurement {
    /// Summary of the generated workload that was measured.
    pub workload_summary: WorkloadSummary,
    /// Ingest operation measurement.
    pub ingest: MeasuredOperation,
    /// Checkout operation measurement.
    pub checkout_operation: MeasuredOperation,
    /// Checkout result counts.
    pub checkout: CheckoutMeasurement,
}

/// Serializable workload summary snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadSummarySnapshot {
    /// Number of generated StateCells.
    pub cell_count: usize,
    /// Number of generated cells marked as frontier.
    pub frontier_count: usize,
    /// Number of generated dependency edges.
    pub dependency_count: usize,
    /// Sum of generated token costs.
    pub total_token_cost: i64,
}

impl From<WorkloadSummary> for WorkloadSummarySnapshot {
    fn from(summary: WorkloadSummary) -> Self {
        Self {
            cell_count: summary.cell_count,
            frontier_count: summary.frontier_count,
            dependency_count: summary.dependency_count,
            total_token_cost: summary.total_token_cost,
        }
    }
}

/// Serializable measured operation snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MeasuredOperationSnapshot {
    /// Number of logical operations performed.
    pub operation_count: usize,
    /// Observed elapsed nanoseconds for the operation group.
    pub elapsed_nanos: u128,
}

impl From<MeasuredOperation> for MeasuredOperationSnapshot {
    fn from(operation: MeasuredOperation) -> Self {
        Self {
            operation_count: operation.operation_count,
            elapsed_nanos: operation.elapsed.as_nanos(),
        }
    }
}

/// Serializable checkout measurement snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckoutMeasurementSnapshot {
    /// Number of checkout candidates matching request constraints.
    pub matched_count: usize,
    /// Number of cells selected into the returned slice.
    pub selected_count: usize,
    /// Number of matching cells omitted as alternatives.
    pub alternative_count: usize,
    /// Number of selected frontier cells.
    pub frontier_count: usize,
    /// Selected token total reported by checkout.
    pub selected_token_count: i64,
}

impl From<CheckoutMeasurement> for CheckoutMeasurementSnapshot {
    fn from(measurement: CheckoutMeasurement) -> Self {
        Self {
            matched_count: measurement.matched_count,
            selected_count: measurement.selected_count,
            alternative_count: measurement.alternative_count,
            frontier_count: measurement.frontier_count,
            selected_token_count: measurement.selected_token_count,
        }
    }
}

/// Serializable file-kernel lookup-plan detail for one indexed constraint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadIndexedConstraintPlanSnapshot {
    /// Stable indexed lookup constraint name.
    pub name: String,
    /// Number of StateCell candidates selected by this single index before intersection.
    pub candidate_count: usize,
}

/// Serializable file-kernel lookup-plan snapshot for workload artifacts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadLookupPlanSnapshot {
    /// Number of indexed lookup constraints present in the request.
    pub indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints present in the request.
    pub indexed_constraints: Vec<String>,
    /// Ordered per-constraint indexed candidate details.
    pub indexed_constraint_plans: Vec<WorkloadIndexedConstraintPlanSnapshot>,
    /// Number of exact lookup constraints present in the request.
    pub exact_constraint_count: usize,
    /// Ordered names of exact lookup constraints checked after candidate selection.
    pub exact_constraints: Vec<String>,
    /// Number of indexed lookup constraints that can over-select candidates.
    pub lossy_indexed_constraint_count: usize,
    /// Ordered names of indexed lookup constraints that require exact residual filtering.
    pub lossy_indexed_constraints: Vec<String>,
    /// Number of StateCell candidates selected before exact predicate filtering.
    pub candidate_count: usize,
    /// Number of selected candidates that satisfy the exact lookup predicate.
    pub exact_match_count: usize,
    /// Number of selected candidates rejected by exact predicate filtering.
    pub filtered_candidate_count: usize,
    /// Exact match share of selected candidates in integer basis points.
    pub candidate_selectivity_basis_points: usize,
    /// Whether lookup must inspect all visible StateCells.
    pub full_scan: bool,
}

impl From<FileKernelLookupPlan> for WorkloadLookupPlanSnapshot {
    fn from(plan: FileKernelLookupPlan) -> Self {
        Self {
            indexed_constraint_count: plan.indexed_constraint_count,
            indexed_constraints: plan
                .indexed_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            indexed_constraint_plans: plan
                .indexed_constraint_plans
                .into_iter()
                .map(|constraint| WorkloadIndexedConstraintPlanSnapshot {
                    name: constraint.name.to_string(),
                    candidate_count: constraint.candidate_count,
                })
                .collect(),
            exact_constraint_count: plan.exact_constraint_count,
            exact_constraints: plan
                .exact_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            lossy_indexed_constraint_count: plan.lossy_indexed_constraint_count,
            lossy_indexed_constraints: plan
                .lossy_indexed_constraints
                .into_iter()
                .map(str::to_string)
                .collect(),
            candidate_count: plan.candidate_count,
            exact_match_count: plan.exact_match_count,
            filtered_candidate_count: plan.filtered_candidate_count,
            candidate_selectivity_basis_points: plan.candidate_selectivity_basis_points,
            full_scan: plan.full_scan,
        }
    }
}

/// Serializable workload measurement snapshot for durable baseline records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadMeasurementSnapshot {
    /// Generated workload summary.
    pub workload: WorkloadSummarySnapshot,
    /// Ingest operation measurement.
    pub ingest: MeasuredOperationSnapshot,
    /// Checkout operation measurement.
    pub checkout_operation: MeasuredOperationSnapshot,
    /// Checkout result counts.
    pub checkout: CheckoutMeasurementSnapshot,
    /// Optional file-kernel lookup-plan diagnostics for the measured checkout request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lookup_plan: Option<WorkloadLookupPlanSnapshot>,
}

impl WorkloadMeasurementSnapshot {
    /// Converts an in-memory measurement into a serializable snapshot.
    pub fn from_measurement(measurement: &WorkloadMeasurement) -> Self {
        Self {
            workload: measurement.workload_summary.into(),
            ingest: measurement.ingest.into(),
            checkout_operation: measurement.checkout_operation.into(),
            checkout: measurement.checkout.into(),
            lookup_plan: None,
        }
    }

    /// Converts an in-memory measurement plus optional file lookup plan into a serializable snapshot.
    pub fn from_measurement_with_lookup_plan(
        measurement: &WorkloadMeasurement,
        lookup_plan: Option<FileKernelLookupPlan>,
    ) -> Self {
        let mut snapshot = Self::from_measurement(measurement);
        snapshot.lookup_plan = lookup_plan.map(Into::into);
        snapshot
    }
}

/// Durable workload measurement baseline record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadBaselineRecord {
    /// Timestamp when this baseline was recorded.
    pub recorded_at: DateTime<Utc>,
    /// Caller-provided scenario label.
    pub label: String,
    /// Caller-provided kernel profile name.
    pub kernel: String,
    /// Serializable measurement snapshot.
    pub snapshot: WorkloadMeasurementSnapshot,
}

impl WorkloadBaselineRecord {
    /// Creates a workload baseline record.
    pub fn new(
        recorded_at: DateTime<Utc>,
        label: impl Into<String>,
        kernel: impl Into<String>,
        snapshot: WorkloadMeasurementSnapshot,
    ) -> Self {
        Self {
            recorded_at,
            label: label.into(),
            kernel: kernel.into(),
            snapshot,
        }
    }
}

/// Regression thresholds used when comparing workload measurements to a baseline.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadRegressionThresholds {
    /// Maximum allowed elapsed-time growth percentage before reporting a regression.
    pub max_elapsed_growth_percent: u128,
}

impl Default for WorkloadRegressionThresholds {
    fn default() -> Self {
        Self {
            max_elapsed_growth_percent: 25,
        }
    }
}

/// Deterministic workload baseline comparison report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkloadBaselineComparison {
    /// Baseline record used for comparison.
    pub baseline: WorkloadBaselineRecord,
    /// Current measurement snapshot compared against the baseline.
    pub current: WorkloadMeasurementSnapshot,
    /// Regressions detected by exact count checks or elapsed-time thresholds.
    pub regressions: Vec<WorkloadBaselineRegression>,
}

impl WorkloadBaselineComparison {
    /// Returns true when no regressions were detected.
    pub fn passed(&self) -> bool {
        self.regressions.is_empty()
    }
}

/// Deterministic workload baseline regression reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorkloadBaselineRegression {
    /// Generated StateCell count changed.
    WorkloadCellCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated frontier StateCell count changed.
    WorkloadFrontierCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated dependency edge count changed.
    WorkloadDependencyCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Generated total token cost changed.
    WorkloadTokenCostChanged {
        /// Baseline token count.
        previous: i64,
        /// Current token count.
        current: i64,
    },
    /// Ingest operation count changed.
    IngestOperationCountChanged {
        /// Baseline operation count.
        previous: usize,
        /// Current operation count.
        current: usize,
    },
    /// Checkout operation count changed.
    CheckoutOperationCountChanged {
        /// Baseline operation count.
        previous: usize,
        /// Current operation count.
        current: usize,
    },
    /// Checkout candidate count changed.
    CheckoutMatchedCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout selected-cell count changed.
    CheckoutSelectedCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout alternative count changed.
    CheckoutAlternativeCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout frontier recommendation count changed.
    CheckoutFrontierCountChanged {
        /// Baseline count.
        previous: usize,
        /// Current count.
        current: usize,
    },
    /// Checkout selected token count changed.
    CheckoutSelectedTokenCountChanged {
        /// Baseline token count.
        previous: i64,
        /// Current token count.
        current: i64,
    },
    /// Ingest elapsed time exceeded the allowed growth threshold.
    IngestElapsedRegressed {
        /// Baseline elapsed nanoseconds.
        previous_nanos: u128,
        /// Current elapsed nanoseconds.
        current_nanos: u128,
        /// Maximum allowed current elapsed nanoseconds under the threshold.
        max_allowed_nanos: u128,
    },
    /// Checkout elapsed time exceeded the allowed growth threshold.
    CheckoutElapsedRegressed {
        /// Baseline elapsed nanoseconds.
        previous_nanos: u128,
        /// Current elapsed nanoseconds.
        current_nanos: u128,
        /// Maximum allowed current elapsed nanoseconds under the threshold.
        max_allowed_nanos: u128,
    },
    /// Lookup-plan diagnostics were added or removed between comparable workload snapshots.
    LookupPlanPresenceChanged {
        /// Whether the baseline snapshot had lookup-plan diagnostics.
        previous: bool,
        /// Whether the current snapshot has lookup-plan diagnostics.
        current: bool,
    },
    /// Ordered indexed lookup constraints changed.
    LookupPlanIndexedConstraintsChanged {
        /// Baseline ordered indexed constraint names.
        previous: Vec<String>,
        /// Current ordered indexed constraint names.
        current: Vec<String>,
    },
    /// Ordered exact lookup constraints changed.
    LookupPlanExactConstraintsChanged {
        /// Baseline ordered exact constraint names.
        previous: Vec<String>,
        /// Current ordered exact constraint names.
        current: Vec<String>,
    },
    /// Ordered lossy indexed lookup constraints changed.
    LookupPlanLossyIndexedConstraintsChanged {
        /// Baseline ordered lossy indexed constraint names.
        previous: Vec<String>,
        /// Current ordered lossy indexed constraint names.
        current: Vec<String>,
    },
    /// Final lookup-plan candidate count changed.
    LookupPlanCandidateCountChanged {
        /// Baseline candidate count.
        previous: usize,
        /// Current candidate count.
        current: usize,
    },
    /// Exact post-filter lookup-plan match count changed.
    LookupPlanExactMatchCountChanged {
        /// Baseline exact match count.
        previous: usize,
        /// Current exact match count.
        current: usize,
    },
    /// Filtered candidate count changed.
    LookupPlanFilteredCandidateCountChanged {
        /// Baseline filtered candidate count.
        previous: usize,
        /// Current filtered candidate count.
        current: usize,
    },
    /// Candidate selectivity changed.
    LookupPlanCandidateSelectivityChanged {
        /// Baseline selectivity in basis points.
        previous: usize,
        /// Current selectivity in basis points.
        current: usize,
    },
    /// Lookup-plan full-scan fallback changed.
    LookupPlanFullScanChanged {
        /// Baseline full-scan status.
        previous: bool,
        /// Current full-scan status.
        current: bool,
    },
    /// Candidate count for a shared indexed lookup constraint changed.
    LookupPlanConstraintCandidateCountChanged {
        /// Stable indexed constraint name.
        name: String,
        /// Baseline candidate count for this constraint.
        previous: usize,
        /// Current candidate count for this constraint.
        current: usize,
    },
}

/// Workload generation failure.
#[derive(Debug, Error)]
pub enum WorkloadError {
    /// Workload must include at least one StateCell.
    #[error("workload must include at least one StateCell")]
    EmptyWorkload,
    /// Semantic anchor prefix must not be empty.
    #[error("anchor prefix must not be empty")]
    EmptyAnchorPrefix,
    /// Project scope must not be empty.
    #[error("project scope must not be empty")]
    EmptyProjectScope,
    /// Frontier interval must be positive.
    #[error("frontier interval must be positive")]
    InvalidFrontierInterval,
    /// Dependency stride must be positive.
    #[error("dependency stride must be positive")]
    InvalidDependencyStride,
    /// Core StateCell construction failed.
    #[error(transparent)]
    Core(#[from] continuitydb_core::CoreError),
}

/// Workload measurement failure.
#[derive(Debug, Error)]
pub enum MeasurementError {
    /// Storage kernel failure while ingesting workload cells.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout failure while materializing a workload slice.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
}

/// Workload baseline store failure.
#[derive(Debug, Error)]
pub enum WorkloadBaselineError {
    /// File I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization failure.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Baseline JSONL record is corrupt.
    #[error("baseline record at line {line} is corrupt")]
    CorruptRecord {
        /// One-based JSONL line number.
        line: usize,
        /// Decode error for the corrupt record.
        #[source]
        source: serde_json::Error,
    },
}

/// Append-only JSONL store for workload measurement baselines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileWorkloadBaselineStore {
    path: PathBuf,
}

impl FileWorkloadBaselineStore {
    /// Creates a file-backed workload baseline store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Appends one baseline record as a JSONL line.
    pub fn append(&self, record: &WorkloadBaselineRecord) -> Result<(), WorkloadBaselineError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, record)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }

    /// Lists baseline records in file order.
    pub fn list(&self) -> Result<Vec<WorkloadBaselineRecord>, WorkloadBaselineError> {
        match File::open(&self.path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut records = Vec::new();
                for (index, line) in reader.lines().enumerate() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }

                    let record = serde_json::from_str(&line).map_err(|source| {
                        WorkloadBaselineError::CorruptRecord {
                            line: index + 1,
                            source,
                        }
                    })?;
                    records.push(record);
                }
                Ok(records)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    /// Returns the newest baseline record matching the given label and kernel.
    pub fn latest_matching(
        &self,
        label: &str,
        kernel: &str,
    ) -> Result<Option<WorkloadBaselineRecord>, WorkloadBaselineError> {
        let mut latest: Option<WorkloadBaselineRecord> = None;
        for record in self.list()? {
            let is_newer_match = match latest.as_ref() {
                Some(current) => record.recorded_at >= current.recorded_at,
                None => true,
            };
            if record.label == label && record.kernel == kernel && is_newer_match {
                latest = Some(record);
            }
        }
        Ok(latest)
    }
}

/// Compares a current workload snapshot to a durable baseline record.
pub fn compare_workload_snapshot_to_baseline(
    baseline: &WorkloadBaselineRecord,
    current: &WorkloadMeasurementSnapshot,
    thresholds: WorkloadRegressionThresholds,
) -> WorkloadBaselineComparison {
    let previous = &baseline.snapshot;
    let mut regressions = Vec::new();

    push_if_changed(
        &mut regressions,
        previous.workload.cell_count,
        current.workload.cell_count,
        |previous, current| WorkloadBaselineRegression::WorkloadCellCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.frontier_count,
        current.workload.frontier_count,
        |previous, current| WorkloadBaselineRegression::WorkloadFrontierCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.dependency_count,
        current.workload.dependency_count,
        |previous, current| WorkloadBaselineRegression::WorkloadDependencyCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.workload.total_token_cost,
        current.workload.total_token_cost,
        |previous, current| WorkloadBaselineRegression::WorkloadTokenCostChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.ingest.operation_count,
        current.ingest.operation_count,
        |previous, current| WorkloadBaselineRegression::IngestOperationCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout_operation.operation_count,
        current.checkout_operation.operation_count,
        |previous, current| WorkloadBaselineRegression::CheckoutOperationCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.matched_count,
        current.checkout.matched_count,
        |previous, current| WorkloadBaselineRegression::CheckoutMatchedCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.selected_count,
        current.checkout.selected_count,
        |previous, current| WorkloadBaselineRegression::CheckoutSelectedCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.alternative_count,
        current.checkout.alternative_count,
        |previous, current| WorkloadBaselineRegression::CheckoutAlternativeCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.frontier_count,
        current.checkout.frontier_count,
        |previous, current| WorkloadBaselineRegression::CheckoutFrontierCountChanged {
            previous,
            current,
        },
    );
    push_if_changed(
        &mut regressions,
        previous.checkout.selected_token_count,
        current.checkout.selected_token_count,
        |previous, current| WorkloadBaselineRegression::CheckoutSelectedTokenCountChanged {
            previous,
            current,
        },
    );

    push_lookup_plan_regressions(
        &mut regressions,
        &previous.lookup_plan,
        &current.lookup_plan,
    );

    let ingest_allowed = max_allowed_elapsed(
        previous.ingest.elapsed_nanos,
        thresholds.max_elapsed_growth_percent,
    );
    if current.ingest.elapsed_nanos > ingest_allowed {
        regressions.push(WorkloadBaselineRegression::IngestElapsedRegressed {
            previous_nanos: previous.ingest.elapsed_nanos,
            current_nanos: current.ingest.elapsed_nanos,
            max_allowed_nanos: ingest_allowed,
        });
    }

    let checkout_allowed = max_allowed_elapsed(
        previous.checkout_operation.elapsed_nanos,
        thresholds.max_elapsed_growth_percent,
    );
    if current.checkout_operation.elapsed_nanos > checkout_allowed {
        regressions.push(WorkloadBaselineRegression::CheckoutElapsedRegressed {
            previous_nanos: previous.checkout_operation.elapsed_nanos,
            current_nanos: current.checkout_operation.elapsed_nanos,
            max_allowed_nanos: checkout_allowed,
        });
    }

    WorkloadBaselineComparison {
        baseline: baseline.clone(),
        current: current.clone(),
        regressions,
    }
}

fn push_if_changed<T, F>(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: T,
    current: T,
    build: F,
) where
    T: Copy + Eq,
    F: FnOnce(T, T) -> WorkloadBaselineRegression,
{
    if previous != current {
        regressions.push(build(previous, current));
    }
}

fn push_lookup_plan_regressions(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: &Option<WorkloadLookupPlanSnapshot>,
    current: &Option<WorkloadLookupPlanSnapshot>,
) {
    match (previous, current) {
        (None, None) => {}
        (None, Some(_)) | (Some(_), None) => {
            regressions.push(WorkloadBaselineRegression::LookupPlanPresenceChanged {
                previous: previous.is_some(),
                current: current.is_some(),
            });
        }
        (Some(previous), Some(current)) => {
            push_if_changed(
                regressions,
                previous.indexed_constraints.as_slice(),
                current.indexed_constraints.as_slice(),
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanIndexedConstraintsChanged {
                        previous: previous.to_vec(),
                        current: current.to_vec(),
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.exact_constraints.as_slice(),
                current.exact_constraints.as_slice(),
                |previous, current| WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: previous.to_vec(),
                    current: current.to_vec(),
                },
            );
            push_if_changed(
                regressions,
                previous.lossy_indexed_constraints.as_slice(),
                current.lossy_indexed_constraints.as_slice(),
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                        previous: previous.to_vec(),
                        current: current.to_vec(),
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.candidate_count,
                current.candidate_count,
                |previous, current| WorkloadBaselineRegression::LookupPlanCandidateCountChanged {
                    previous,
                    current,
                },
            );
            push_if_changed(
                regressions,
                previous.exact_match_count,
                current.exact_match_count,
                |previous, current| WorkloadBaselineRegression::LookupPlanExactMatchCountChanged {
                    previous,
                    current,
                },
            );
            push_if_changed(
                regressions,
                previous.filtered_candidate_count,
                current.filtered_candidate_count,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanFilteredCandidateCountChanged {
                        previous,
                        current,
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.candidate_selectivity_basis_points,
                current.candidate_selectivity_basis_points,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanCandidateSelectivityChanged {
                        previous,
                        current,
                    }
                },
            );
            push_if_changed(
                regressions,
                previous.full_scan,
                current.full_scan,
                |previous, current| WorkloadBaselineRegression::LookupPlanFullScanChanged {
                    previous,
                    current,
                },
            );
            push_lookup_plan_constraint_candidate_count_regressions(
                regressions,
                &previous.indexed_constraint_plans,
                &current.indexed_constraint_plans,
            );
        }
    }
}

fn push_lookup_plan_constraint_candidate_count_regressions(
    regressions: &mut Vec<WorkloadBaselineRegression>,
    previous: &[WorkloadIndexedConstraintPlanSnapshot],
    current: &[WorkloadIndexedConstraintPlanSnapshot],
) {
    let current_counts = current
        .iter()
        .map(|plan| (plan.name.as_str(), plan.candidate_count))
        .collect::<BTreeMap<_, _>>();

    for previous_plan in previous {
        if let Some(current_count) = current_counts.get(previous_plan.name.as_str()) {
            push_if_changed(
                regressions,
                previous_plan.candidate_count,
                *current_count,
                |previous, current| {
                    WorkloadBaselineRegression::LookupPlanConstraintCandidateCountChanged {
                        name: previous_plan.name.clone(),
                        previous,
                        current,
                    }
                },
            );
        }
    }
}

fn max_allowed_elapsed(previous_nanos: u128, growth_percent: u128) -> u128 {
    previous_nanos + ((previous_nanos * growth_percent) / 100)
}

/// Generates a deterministic world-model workload for benchmarks and engine comparisons.
pub fn generate_world_model_workload(
    config: WorkloadConfig,
) -> Result<ContinuityWorkload, WorkloadError> {
    validate_config(&config)?;

    let anchor_prefix = config.anchor_prefix.trim();
    let project_scope = config.project_scope.trim();
    let mut cells: Vec<StateCell> = Vec::with_capacity(config.cell_count);
    let mut frontier_count = 0;
    let mut dependency_count = 0;
    let mut total_token_cost = 0;

    for index in 0..config.cell_count {
        let id = StateCellId::from_u128(config.id_seed + index as u128);
        let token_count = 120 + (index % 17) as i64;
        let mut cell = StateCell::new(
            id,
            vec![SemanticAnchor::new(format!(
                "{anchor_prefix}:cell:{index:06}"
            ))],
            ValidTimeRange::new(config.valid_from, None)?,
            Scope::Project(project_scope.to_string()),
            Answerability::new(vec![format!(
                "what is the operational state for {anchor_prefix} cell {index}?"
            )])?,
            vec![Evidence {
                source: continuitydb_core::SourceId::new(format!("workload:source:{index:06}")),
                citation: Citation {
                    locator: format!("workload://{anchor_prefix}/evidence/{index:06}"),
                },
                confidence: Confidence::new(confidence_for(index))?,
                trust: vec![trust_signal_for(index)],
            }],
            CellPayload::Text(format!(
                "Deterministic workload cell {index} for {anchor_prefix}."
            )),
            CellCost::new(token_count, (index % 5) as i64)?,
        )?;

        cell.utility_feedback = utility_feedback_for(index)?;

        if (index + 1) % config.frontier_every == 0 {
            cell.activation = ActivationState::Frontier;
            frontier_count += 1;
        }

        if index >= config.dependency_stride {
            cell.dependencies.push(CellDependency::new(
                cells[index - config.dependency_stride].id,
                CellDependencyKind::DependsOn,
                format!(
                    "workload cell {index} depends on cell {}",
                    index - config.dependency_stride
                ),
            ));
            dependency_count += 1;
        }

        total_token_cost += token_count;
        cells.push(cell);
    }

    Ok(ContinuityWorkload {
        cells,
        summary: WorkloadSummary {
            cell_count: config.cell_count,
            frontier_count,
            dependency_count,
            total_token_cost,
        },
    })
}

/// Measures ingest and checkout over a deterministic workload using any storage kernel.
pub fn measure_ingest_and_checkout<K>(
    kernel: &mut K,
    workload: &ContinuityWorkload,
    committed_at: DateTime<Utc>,
    request: CheckoutRequest,
) -> Result<WorkloadMeasurement, MeasurementError>
where
    K: StorageKernel,
{
    let ingest_started = Instant::now();
    kernel.append_cells_at(workload.cells.clone(), committed_at)?;
    let ingest_elapsed = ingest_started.elapsed();

    let checkout_started = Instant::now();
    let slice = checkout(kernel, request)?;
    let checkout_elapsed = checkout_started.elapsed();

    Ok(WorkloadMeasurement {
        workload_summary: workload.summary,
        ingest: MeasuredOperation {
            operation_count: workload.cells.len(),
            elapsed: ingest_elapsed,
        },
        checkout_operation: MeasuredOperation {
            operation_count: 1,
            elapsed: checkout_elapsed,
        },
        checkout: CheckoutMeasurement {
            matched_count: slice.cells.len() + slice.alternatives.len(),
            selected_count: slice.cells.len(),
            alternative_count: slice.alternatives.len(),
            frontier_count: slice.frontier_recommendations.len(),
            selected_token_count: slice.total_tokens,
        },
    })
}

fn validate_config(config: &WorkloadConfig) -> Result<(), WorkloadError> {
    if config.cell_count == 0 {
        return Err(WorkloadError::EmptyWorkload);
    }

    if config.anchor_prefix.trim().is_empty() {
        return Err(WorkloadError::EmptyAnchorPrefix);
    }

    if config.project_scope.trim().is_empty() {
        return Err(WorkloadError::EmptyProjectScope);
    }

    if config.frontier_every == 0 {
        return Err(WorkloadError::InvalidFrontierInterval);
    }

    if config.dependency_stride == 0 {
        return Err(WorkloadError::InvalidDependencyStride);
    }

    Ok(())
}

fn confidence_for(index: usize) -> f32 {
    0.55 + ((index % 9) as f32 * 0.05)
}

fn trust_signal_for(index: usize) -> TrustSignal {
    match index % 3 {
        0 => TrustSignal::DirectObservation,
        1 => TrustSignal::HumanSupplied,
        _ => TrustSignal::Derived,
    }
}

fn utility_feedback_for(index: usize) -> Result<UtilityFeedback, WorkloadError> {
    Ok(UtilityFeedback::new(
        Confidence::new(0.45 + ((index % 5) as f32 * 0.1))?,
        Confidence::new(0.5 + ((index % 4) as f32 * 0.1))?,
        Confidence::new(0.4 + ((index % 6) as f32 * 0.08))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use continuitydb_checkout::CheckoutRequest;
    use continuitydb_core::{ActivationState, CellDependencyKind, Confidence, Scope};
    use continuitydb_memory::MemoryKernel;
    use std::fs;

    fn sample_config() -> Result<WorkloadConfig, Box<dyn std::error::Error>> {
        let valid_from = Utc
            .with_ymd_and_hms(2026, 5, 20, 0, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;

        Ok(WorkloadConfig {
            cell_count: 8,
            id_seed: 1_000,
            anchor_prefix: "bench:world".to_string(),
            project_scope: "continuitydb".to_string(),
            valid_from,
            frontier_every: 3,
            dependency_stride: 2,
        })
    }

    #[test]
    fn workload_generation_is_repeatable() -> Result<(), Box<dyn std::error::Error>> {
        let first = generate_world_model_workload(sample_config()?)?;
        let second = generate_world_model_workload(sample_config()?)?;

        assert_eq!(first, second);
        assert_eq!(first.summary.cell_count, 8);
        assert_eq!(
            first.cells[0].id.to_string(),
            "00000000-0000-0000-0000-0000000003e8"
        );
        assert_eq!(
            first.cells[0].anchors[0].as_str(),
            "bench:world:cell:000000"
        );
        assert_eq!(
            first.cells[0].answerability.questions(),
            ["what is the operational state for bench:world cell 0?"]
        );
        Ok(())
    }

    #[test]
    fn workload_generation_covers_frontier_dependencies_evidence_and_cost(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;

        assert_eq!(workload.summary.frontier_count, 2);
        assert_eq!(workload.summary.dependency_count, 6);
        assert_eq!(workload.summary.total_token_cost, 988);
        assert_eq!(workload.cells[2].activation, ActivationState::Frontier);
        assert_eq!(workload.cells[5].activation, ActivationState::Frontier);
        assert_eq!(
            workload.cells[0].scope,
            Scope::Project("continuitydb".to_string())
        );
        assert_eq!(
            workload.cells[2].dependencies[0].target,
            workload.cells[0].id
        );
        assert_eq!(
            workload.cells[2].dependencies[0].kind,
            CellDependencyKind::DependsOn
        );
        assert_eq!(
            workload.cells[3].evidence[0].source.as_str(),
            "workload:source:000003"
        );
        assert_eq!(workload.cells[4].cost.token_count, 124);
        Ok(())
    }

    #[test]
    fn workload_generation_rejects_invalid_config() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = sample_config()?;
        config.cell_count = 0;

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::EmptyWorkload)
        ));

        let mut config = sample_config()?;
        config.anchor_prefix = "  ".to_string();

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::EmptyAnchorPrefix)
        ));

        let mut config = sample_config()?;
        config.frontier_every = 0;

        assert!(matches!(
            generate_world_model_workload(config),
            Err(WorkloadError::InvalidFrontierInterval)
        ));

        Ok(())
    }

    #[test]
    fn workload_measurement_reports_memory_ingest_and_checkout_counts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        let measurement =
            measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)?;

        assert_eq!(measurement.workload_summary, workload.summary);
        assert_eq!(measurement.ingest.operation_count, 8);
        assert_eq!(measurement.checkout.matched_count, 8);
        assert_eq!(measurement.checkout.selected_count, 3);
        assert_eq!(measurement.checkout.alternative_count, 5);
        assert_eq!(measurement.checkout.frontier_count, 0);
        assert!(measurement.checkout.selected_token_count <= 400);
        Ok(())
    }

    #[test]
    fn workload_measurement_reports_frontier_checkout_counts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: Some(ActivationState::Frontier),
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        let measurement =
            measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)?;

        assert_eq!(measurement.checkout.matched_count, 2);
        assert_eq!(measurement.checkout.selected_count, 2);
        assert_eq!(measurement.checkout.alternative_count, 0);
        assert_eq!(measurement.checkout.frontier_count, 2);
        Ok(())
    }

    #[test]
    fn workload_measurement_reports_duplicate_ingest_errors(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request.clone())?;
        let result = measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request);

        assert!(matches!(
            result,
            Err(MeasurementError::Kernel(
                continuitydb_kernel::KernelError::DuplicateCell
            ))
        ));
        Ok(())
    }

    #[test]
    fn workload_baseline_snapshot_preserves_counts_and_elapsed_nanos(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement(&measurement);

        assert_eq!(snapshot.workload.cell_count, 8);
        assert_eq!(snapshot.workload.frontier_count, 2);
        assert_eq!(snapshot.ingest.operation_count, 8);
        assert!(snapshot.ingest.elapsed_nanos > 0);
        assert_eq!(snapshot.checkout.matched_count, 8);
        assert_eq!(snapshot.checkout.selected_count, 3);
        assert_eq!(snapshot.checkout.alternative_count, 5);
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_file_lookup_plan() -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 2,
                indexed_constraints: vec!["scope", "minimum_confidence"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 8,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "minimum_confidence",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 2,
                exact_constraints: vec!["scope", "minimum_confidence"],
                lossy_indexed_constraint_count: 0,
                lossy_indexed_constraints: Vec::new(),
                candidate_count: 8,
                exact_match_count: 8,
                filtered_candidate_count: 0,
                candidate_selectivity_basis_points: 10000,
                full_scan: false,
            }),
        );
        let lookup_plan = snapshot
            .lookup_plan
            .ok_or_else(|| std::io::Error::other("lookup plan was not captured"))?;

        assert_eq!(lookup_plan.indexed_constraint_count, 2);
        assert_eq!(
            lookup_plan.indexed_constraints,
            vec!["scope", "minimum_confidence"]
        );
        assert_eq!(lookup_plan.indexed_constraint_plans[0].candidate_count, 8);
        assert_eq!(lookup_plan.candidate_count, 8);
        assert!(!lookup_plan.full_scan);
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_exact_match_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(json["lookup_plan"]["exact_match_count"].as_u64(), Some(5));
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_filtered_candidate_count(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["filtered_candidate_count"].as_u64(),
            Some(3)
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_candidate_selectivity(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 1,
                indexed_constraints: vec!["valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 1,
                exact_constraints: vec!["valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 8,
                exact_match_count: 5,
                filtered_candidate_count: 3,
                candidate_selectivity_basis_points: 6250,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["candidate_selectivity_basis_points"].as_u64(),
            Some(6250)
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_exact_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 2,
                indexed_constraints: vec!["scope", "valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 5,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 2,
                exact_constraints: vec!["scope", "valid_at"],
                lossy_indexed_constraint_count: 1,
                lossy_indexed_constraints: vec!["valid_at"],
                candidate_count: 5,
                exact_match_count: 4,
                filtered_candidate_count: 1,
                candidate_selectivity_basis_points: 8000,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["exact_constraint_count"].as_u64(),
            Some(2)
        );
        assert_eq!(
            json["lookup_plan"]["exact_constraints"],
            serde_json::json!(["scope", "valid_at"])
        );
        Ok(())
    }

    #[test]
    fn workload_snapshot_preserves_lookup_plan_lossy_indexed_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let measurement = sample_measurement()?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement_with_lookup_plan(
            &measurement,
            Some(continuitydb_kernel::FileKernelLookupPlan {
                indexed_constraint_count: 3,
                indexed_constraints: vec!["scope", "system_at", "valid_at"],
                indexed_constraint_plans: vec![
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "scope",
                        candidate_count: 5,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "system_at",
                        candidate_count: 8,
                    },
                    continuitydb_kernel::FileKernelIndexedConstraintPlan {
                        name: "valid_at",
                        candidate_count: 8,
                    },
                ],
                exact_constraint_count: 3,
                exact_constraints: vec!["scope", "system_at", "valid_at"],
                lossy_indexed_constraint_count: 2,
                lossy_indexed_constraints: vec!["system_at", "valid_at"],
                candidate_count: 5,
                exact_match_count: 4,
                filtered_candidate_count: 1,
                candidate_selectivity_basis_points: 8000,
                full_scan: false,
            }),
        );
        let json = serde_json::to_value(snapshot)?;

        assert_eq!(
            json["lookup_plan"]["lossy_indexed_constraint_count"].as_u64(),
            Some(2)
        );
        assert_eq!(
            json["lookup_plan"]["lossy_indexed_constraints"],
            serde_json::json!(["system_at", "valid_at"])
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_store_appends_and_lists_in_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_baseline_path("continuitydb-workload-baseline-order");
        let store = FileWorkloadBaselineStore::new(&path);
        let recorded_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 2, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let snapshot = WorkloadMeasurementSnapshot::from_measurement(&sample_measurement()?);
        let first =
            WorkloadBaselineRecord::new(recorded_at, "small-memory", "memory", snapshot.clone());
        let second = WorkloadBaselineRecord::new(recorded_at, "small-file", "file", snapshot);

        store.append(&first)?;
        store.append(&second)?;
        let records = store.list()?;

        assert_eq!(records, vec![first, second]);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_store_lists_missing_file_as_empty(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_baseline_path("continuitydb-workload-baseline-missing");
        let store = FileWorkloadBaselineStore::new(&path);

        assert!(store.list()?.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_store_reports_corrupt_jsonl_line() -> Result<(), Box<dyn std::error::Error>>
    {
        let path = temp_baseline_path("continuitydb-workload-baseline-corrupt");
        fs::write(&path, "{not-json}\n")?;
        let store = FileWorkloadBaselineStore::new(&path);

        assert!(matches!(
            store.list(),
            Err(WorkloadBaselineError::CorruptRecord { line: 1, .. })
        ));

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_latest_matching_uses_label_kernel_and_timestamp(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = temp_baseline_path("continuitydb-workload-baseline-latest");
        let store = FileWorkloadBaselineStore::new(&path);
        let snapshot = deterministic_snapshot();
        let older = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 2)?,
            "target",
            "memory",
            snapshot.clone(),
        );
        let newest_matching = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 3)?,
            "target",
            "memory",
            snapshot.clone(),
        );
        let newer_different_kernel = WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 4)?,
            "target",
            "file",
            snapshot.clone(),
        );
        let newer_different_label =
            WorkloadBaselineRecord::new(timestamp(2026, 5, 20, 5)?, "other", "memory", snapshot);

        store.append(&older)?;
        store.append(&newest_matching)?;
        store.append(&newer_different_kernel)?;
        store.append(&newer_different_label)?;

        assert_eq!(
            store.latest_matching("target", "memory")?,
            Some(newest_matching)
        );
        assert_eq!(store.latest_matching("missing", "memory")?, None);

        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_passes_equal_counts_within_elapsed_threshold(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let current = baseline_record("current", "memory", 125, 120)?;
        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current.snapshot,
            WorkloadRegressionThresholds {
                max_elapsed_growth_percent: 25,
            },
        );

        assert!(comparison.passed());
        assert!(comparison.regressions.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_reports_count_and_elapsed_changes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.workload.cell_count += 1;
        current_snapshot.workload.frontier_count += 1;
        current_snapshot.workload.dependency_count += 1;
        current_snapshot.workload.total_token_cost += 10;
        current_snapshot.ingest.operation_count += 1;
        current_snapshot.ingest.elapsed_nanos = 126;
        current_snapshot.checkout_operation.operation_count += 1;
        current_snapshot.checkout_operation.elapsed_nanos = 150;
        current_snapshot.checkout.matched_count += 1;
        current_snapshot.checkout.selected_count += 1;
        current_snapshot.checkout.alternative_count += 1;
        current_snapshot.checkout.frontier_count += 1;
        current_snapshot.checkout.selected_token_count += 10;

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds {
                max_elapsed_growth_percent: 25,
            },
        );

        assert!(!comparison.passed());
        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::WorkloadCellCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::WorkloadFrontierCountChanged {
                    previous: 2,
                    current: 3
                },
                WorkloadBaselineRegression::WorkloadDependencyCountChanged {
                    previous: 6,
                    current: 7
                },
                WorkloadBaselineRegression::WorkloadTokenCostChanged {
                    previous: 988,
                    current: 998
                },
                WorkloadBaselineRegression::IngestOperationCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::CheckoutOperationCountChanged {
                    previous: 1,
                    current: 2
                },
                WorkloadBaselineRegression::CheckoutMatchedCountChanged {
                    previous: 8,
                    current: 9
                },
                WorkloadBaselineRegression::CheckoutSelectedCountChanged {
                    previous: 3,
                    current: 4
                },
                WorkloadBaselineRegression::CheckoutAlternativeCountChanged {
                    previous: 5,
                    current: 6
                },
                WorkloadBaselineRegression::CheckoutFrontierCountChanged {
                    previous: 0,
                    current: 1
                },
                WorkloadBaselineRegression::CheckoutSelectedTokenCountChanged {
                    previous: 370,
                    current: 380
                },
                WorkloadBaselineRegression::IngestElapsedRegressed {
                    previous_nanos: 100,
                    current_nanos: 126,
                    max_allowed_nanos: 125
                },
                WorkloadBaselineRegression::CheckoutElapsedRegressed {
                    previous_nanos: 100,
                    current_nanos: 150,
                    max_allowed_nanos: 125
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_presence_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "file", 100, 100)?;
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan =
            Some(lookup_plan_snapshot(&["scope"], &[("scope", 8)], 8, false));

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![WorkloadBaselineRegression::LookupPlanPresenceChanged {
                previous: false,
                current: true
            }]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_ignores_missing_lookup_plans(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let previous = baseline_record("current", "memory", 100, 100)?;
        let current_snapshot = deterministic_snapshot();

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert!(comparison.passed());
        assert!(comparison.regressions.is_empty());
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_changes(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(lookup_plan_snapshot(
            &["scope", "minimum_confidence"],
            &[("scope", 8), ("minimum_confidence", 8)],
            8,
            false,
        ));
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(lookup_plan_snapshot(
            &["scope", "activation"],
            &[("scope", 7), ("activation", 9)],
            9,
            true,
        ));

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanIndexedConstraintsChanged {
                    previous: vec!["scope".to_string(), "minimum_confidence".to_string()],
                    current: vec!["scope".to_string(), "activation".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: vec!["scope".to_string(), "minimum_confidence".to_string()],
                    current: vec!["scope".to_string(), "activation".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanCandidateCountChanged {
                    previous: 8,
                    current: 9,
                },
                WorkloadBaselineRegression::LookupPlanExactMatchCountChanged {
                    previous: 8,
                    current: 9,
                },
                WorkloadBaselineRegression::LookupPlanFullScanChanged {
                    previous: false,
                    current: true,
                },
                WorkloadBaselineRegression::LookupPlanConstraintCandidateCountChanged {
                    name: "scope".to_string(),
                    previous: 8,
                    current: 7,
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_filtered_candidate_count_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 4,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanFilteredCandidateCountChanged {
                    previous: 3,
                    current: 4,
                }
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_exact_constraint_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["scope".to_string()],
            lossy_indexed_constraint_count: 0,
            lossy_indexed_constraints: Vec::new(),
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 2,
            exact_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanExactConstraintsChanged {
                    previous: vec!["scope".to_string()],
                    current: vec!["scope".to_string(), "valid_at".to_string()],
                },
                WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                    previous: Vec::new(),
                    current: vec!["valid_at".to_string()],
                }
            ]
        );
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_lossy_constraint_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["scope".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "scope".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["scope".to_string()],
            lossy_indexed_constraint_count: 0,
            lossy_indexed_constraints: Vec::new(),
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 2,
            indexed_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            indexed_constraint_plans: vec![
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "scope".to_string(),
                    candidate_count: 8,
                },
                WorkloadIndexedConstraintPlanSnapshot {
                    name: "valid_at".to_string(),
                    candidate_count: 8,
                },
            ],
            exact_constraint_count: 2,
            exact_constraints: vec!["scope".to_string(), "valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 8,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: 10000,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert!(comparison.regressions.contains(
            &WorkloadBaselineRegression::LookupPlanLossyIndexedConstraintsChanged {
                previous: Vec::new(),
                current: vec!["valid_at".to_string()],
            }
        ));
        Ok(())
    }

    #[test]
    fn workload_baseline_regression_detects_lookup_plan_candidate_selectivity_change(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut previous = baseline_record("current", "file", 100, 100)?;
        previous.snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 6250,
            full_scan: false,
        });
        let mut current_snapshot = deterministic_snapshot();
        current_snapshot.lookup_plan = Some(WorkloadLookupPlanSnapshot {
            indexed_constraint_count: 1,
            indexed_constraints: vec!["valid_at".to_string()],
            indexed_constraint_plans: vec![WorkloadIndexedConstraintPlanSnapshot {
                name: "valid_at".to_string(),
                candidate_count: 8,
            }],
            exact_constraint_count: 1,
            exact_constraints: vec!["valid_at".to_string()],
            lossy_indexed_constraint_count: 1,
            lossy_indexed_constraints: vec!["valid_at".to_string()],
            candidate_count: 8,
            exact_match_count: 5,
            filtered_candidate_count: 3,
            candidate_selectivity_basis_points: 7500,
            full_scan: false,
        });

        let comparison = compare_workload_snapshot_to_baseline(
            &previous,
            &current_snapshot,
            WorkloadRegressionThresholds::default(),
        );

        assert_eq!(
            comparison.regressions,
            vec![
                WorkloadBaselineRegression::LookupPlanCandidateSelectivityChanged {
                    previous: 6250,
                    current: 7500,
                }
            ]
        );
        Ok(())
    }

    fn sample_measurement() -> Result<WorkloadMeasurement, Box<dyn std::error::Error>> {
        let workload = generate_world_model_workload(sample_config()?)?;
        let mut kernel = MemoryKernel::default();
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 1, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let request = CheckoutRequest {
            semantic_anchor: None,
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: None,
            system_at: None,
            commit_id: None,
            activation: None,
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.0)?,
            token_budget: 400,
        };

        measure_ingest_and_checkout(&mut kernel, &workload, committed_at, request)
            .map_err(Into::into)
    }

    fn baseline_record(
        label: &str,
        kernel: &str,
        ingest_elapsed_nanos: u128,
        checkout_elapsed_nanos: u128,
    ) -> Result<WorkloadBaselineRecord, Box<dyn std::error::Error>> {
        let mut snapshot = deterministic_snapshot();
        snapshot.ingest.elapsed_nanos = ingest_elapsed_nanos;
        snapshot.checkout_operation.elapsed_nanos = checkout_elapsed_nanos;
        Ok(WorkloadBaselineRecord::new(
            timestamp(2026, 5, 20, 2)?,
            label,
            kernel,
            snapshot,
        ))
    }

    fn deterministic_snapshot() -> WorkloadMeasurementSnapshot {
        WorkloadMeasurementSnapshot {
            workload: WorkloadSummarySnapshot {
                cell_count: 8,
                frontier_count: 2,
                dependency_count: 6,
                total_token_cost: 988,
            },
            ingest: MeasuredOperationSnapshot {
                operation_count: 8,
                elapsed_nanos: 100,
            },
            checkout_operation: MeasuredOperationSnapshot {
                operation_count: 1,
                elapsed_nanos: 100,
            },
            checkout: CheckoutMeasurementSnapshot {
                matched_count: 8,
                selected_count: 3,
                alternative_count: 5,
                frontier_count: 0,
                selected_token_count: 370,
            },
            lookup_plan: None,
        }
    }

    fn lookup_plan_snapshot(
        constraints: &[&str],
        constraint_plans: &[(&str, usize)],
        candidate_count: usize,
        full_scan: bool,
    ) -> WorkloadLookupPlanSnapshot {
        WorkloadLookupPlanSnapshot {
            indexed_constraint_count: constraints.len(),
            indexed_constraints: constraints
                .iter()
                .map(|constraint| constraint.to_string())
                .collect(),
            indexed_constraint_plans: constraint_plans
                .iter()
                .map(
                    |(name, candidate_count)| WorkloadIndexedConstraintPlanSnapshot {
                        name: name.to_string(),
                        candidate_count: *candidate_count,
                    },
                )
                .collect(),
            exact_constraint_count: constraints.len(),
            exact_constraints: constraints
                .iter()
                .map(|constraint| constraint.to_string())
                .collect(),
            lossy_indexed_constraint_count: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .count(),
            lossy_indexed_constraints: constraints
                .iter()
                .filter(|constraint| **constraint == "system_at" || **constraint == "valid_at")
                .map(|constraint| constraint.to_string())
                .collect(),
            candidate_count,
            exact_match_count: candidate_count,
            filtered_candidate_count: 0,
            candidate_selectivity_basis_points: if candidate_count == 0 { 0 } else { 10000 },
            full_scan,
        }
    }

    fn timestamp(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
    ) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp").into())
    }

    fn temp_baseline_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{name}-{:?}.jsonl", StateCellId::new()))
    }
}
