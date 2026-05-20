//! Native embeddable operation API for ContinuityDB.

use chrono::{DateTime, Utc};
use continuitydb_checkout::{
    audit, checkout, AuditTrace, CheckoutError, CheckoutRequest, CheckoutSlice,
};
use continuitydb_core::{StateCell, StateCellId};
use continuitydb_kernel::{CellLookup, KernelError, StorageKernel};
use thiserror::Error;

/// Errors produced by the native ContinuityDB operation API.
#[derive(Debug, Error, PartialEq)]
pub enum ContinuityError {
    /// Storage kernel failure.
    #[error(transparent)]
    Kernel(#[from] KernelError),
    /// Checkout operation failure.
    #[error(transparent)]
    Checkout(#[from] CheckoutError),
    /// Requested StateCell was not found in the backing kernel.
    #[error("state cell not found")]
    CellNotFound {
        /// Missing cell identifier.
        cell_id: StateCellId,
    },
}

/// Native embeddable ContinuityDB operation boundary.
#[derive(Clone, Debug)]
pub struct ContinuityDb<K> {
    kernel: K,
}

impl<K> ContinuityDb<K> {
    /// Creates a database wrapper over an existing storage kernel.
    pub fn new(kernel: K) -> Self {
        Self { kernel }
    }

    /// Returns the backing storage kernel.
    pub fn kernel(&self) -> &K {
        &self.kernel
    }

    /// Returns mutable access to the backing storage kernel.
    pub fn kernel_mut(&mut self) -> &mut K {
        &mut self.kernel
    }

    /// Consumes the API wrapper and returns the backing storage kernel.
    pub fn into_kernel(self) -> K {
        self.kernel
    }
}

impl<K: StorageKernel> ContinuityDb<K> {
    /// Appends an immutable StateCell version and returns its identifier.
    pub fn ingest_cell(&mut self, cell: StateCell) -> Result<StateCellId, ContinuityError> {
        let cell_id = cell.id;
        self.kernel.append_cell(cell)?;
        Ok(cell_id)
    }

    /// Appends an immutable StateCell version at a deterministic system time.
    pub fn ingest_cell_at(
        &mut self,
        cell: StateCell,
        committed_at: DateTime<Utc>,
    ) -> Result<StateCellId, ContinuityError> {
        let cell_id = cell.id;
        self.kernel.append_cell_at(cell, committed_at)?;
        Ok(cell_id)
    }

    /// Materializes a deterministic continuity slice.
    pub fn checkout(&self, request: CheckoutRequest) -> Result<CheckoutSlice, ContinuityError> {
        checkout(&self.kernel, request).map_err(Into::into)
    }

    /// Produces an audit trace for a stored StateCell.
    pub fn audit_cell(&self, cell_id: StateCellId) -> Result<AuditTrace, ContinuityError> {
        let cells = self.kernel.lookup_cells(CellLookup {
            cell_id: Some(cell_id),
            ..CellLookup::default()
        })?;

        cells
            .first()
            .map(audit)
            .ok_or(ContinuityError::CellNotFound { cell_id })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use continuitydb_checkout::CheckoutRequest;
    use continuitydb_core::{
        Answerability, CellCost, CellPayload, Citation, Confidence, Evidence, Scope,
        SemanticAnchor, SourceId, StateCell, StateCellId, TrustSignal, ValidTimeRange,
    };
    use continuitydb_memory::MemoryKernel;

    use super::{ContinuityDb, ContinuityError};

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
    fn api_ingests_and_checkouts_cells() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:api", 0.91, 12)?;
        let cell_id = cell.id;

        let returned_id = db.ingest_cell_at(cell, committed_at)?;
        let slice = db.checkout(CheckoutRequest {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(committed_at),
            system_at: Some(committed_at),
            answerability_question: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: Confidence::new(0.8)?,
            token_budget: 100,
        })?;

        assert_eq!(returned_id, cell_id);
        assert_eq!(slice.cells.len(), 1);
        assert_eq!(slice.cells[0].id, cell_id);
        assert_eq!(slice.cells[0].system_time.from(), committed_at);
        Ok(())
    }

    #[test]
    fn api_audits_cell_by_id_with_structured_evidence() -> Result<(), Box<dyn std::error::Error>> {
        let committed_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let mut db = ContinuityDb::new(MemoryKernel::default());
        let cell = sample_cell("project:continuitydb:audit-api", 0.91, 12)?;
        let cell_id = db.ingest_cell_at(cell, committed_at)?;

        let trace = db.audit_cell(cell_id)?;

        assert_eq!(trace.cell_id, cell_id);
        assert_eq!(trace.evidence.len(), 1);
        assert_eq!(trace.evidence[0].source, "test");
        assert_eq!(
            trace.evidence[0].locator,
            "test://project:continuitydb:audit-api"
        );
        assert_eq!(trace.evidence[0].confidence, Confidence::new(0.91)?);
        assert_eq!(
            trace.evidence[0].trust,
            vec![TrustSignal::DirectObservation]
        );
        Ok(())
    }

    #[test]
    fn api_audit_cell_reports_missing_id() {
        let db = ContinuityDb::new(MemoryKernel::default());
        let missing_id = StateCellId::new();

        let result = db.audit_cell(missing_id);

        assert!(matches!(
            result,
            Err(ContinuityError::CellNotFound { cell_id }) if cell_id == missing_id
        ));
    }
}
