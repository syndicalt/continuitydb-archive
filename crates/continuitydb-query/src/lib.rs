//! Typed ContinuityDB query AST.

mod text;

use chrono::{DateTime, Utc};
use continuitydb_checkout::CheckoutRequest;
use continuitydb_core::{CellDependencyKind, CommitId, Confidence, Scope, StateCellId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use text::{parse_query_text, QueryTextError};

/// Wire-format marker for versioned query JSON envelopes.
pub const QUERY_ENVELOPE_FORMAT: &str = "continuitydb.query";
/// Supported query JSON envelope version.
pub const QUERY_ENVELOPE_FORMAT_VERSION: u32 = 1;

/// Versioned JSON envelope for portable typed query files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryEnvelope {
    /// Wire-format marker.
    pub format: String,
    /// Wire-format version.
    pub version: u32,
    /// Serialized typed query.
    pub query: ContinuityQuery,
}

impl QueryEnvelope {
    /// Wraps a typed query in the current JSON envelope.
    pub fn new(query: ContinuityQuery) -> Self {
        Self {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION,
            query,
        }
    }

    /// Validates the envelope format and version.
    pub fn validate(&self) -> Result<(), QueryEnvelopeError> {
        if self.format == QUERY_ENVELOPE_FORMAT && self.version == QUERY_ENVELOPE_FORMAT_VERSION {
            Ok(())
        } else {
            Err(QueryEnvelopeError::InvalidEnvelope)
        }
    }
}

/// Top-level typed ContinuityDB query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuityQuery {
    /// Materialize a continuity checkout slice.
    Checkout(CheckoutQuery),
}

impl ContinuityQuery {
    /// Compiles this query into a deterministic checkout request.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        match self {
            Self::Checkout(query) => query.compile_checkout(),
        }
    }
}

/// Structured checkout query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckoutQuery {
    task: QueryTask,
    requirements: QueryRequirements,
    return_shape: QueryReturnShape,
    optimization: QueryOptimization,
}

impl CheckoutQuery {
    /// Creates a checkout query with default requirements and supported output semantics.
    pub fn new(task: QueryTask) -> Self {
        Self {
            task,
            requirements: QueryRequirements::default(),
            return_shape: QueryReturnShape::PackedContextWithMetadata,
            optimization: QueryOptimization::DeterministicUtility,
        }
    }

    /// Returns query task identity and answerability intent.
    pub fn task(&self) -> &QueryTask {
        &self.task
    }

    /// Returns deterministic checkout requirements.
    pub fn requirements(&self) -> &QueryRequirements {
        &self.requirements
    }

    /// Returns the requested materialization shape.
    pub fn return_shape(&self) -> QueryReturnShape {
        self.return_shape
    }

    /// Returns the requested optimization policy.
    pub fn optimization(&self) -> QueryOptimization {
        self.optimization
    }

    /// Replaces deterministic checkout requirements.
    pub fn with_requirements(mut self, requirements: QueryRequirements) -> Self {
        self.requirements = requirements;
        self
    }

    /// Replaces the requested materialization shape.
    pub fn with_return_shape(mut self, return_shape: QueryReturnShape) -> Self {
        self.return_shape = return_shape;
        self
    }

    /// Replaces the requested optimization policy.
    pub fn with_optimization(mut self, optimization: QueryOptimization) -> Self {
        self.optimization = optimization;
        self
    }

    /// Compiles this query into the current deterministic checkout request type.
    pub fn compile_checkout(self) -> Result<CheckoutRequest, QueryError> {
        if self.return_shape != QueryReturnShape::PackedContextWithMetadata {
            return Err(QueryError::UnsupportedReturnShape(self.return_shape));
        }
        if self.optimization != QueryOptimization::DeterministicUtility {
            return Err(QueryError::UnsupportedOptimization(self.optimization));
        }

        Ok(CheckoutRequest {
            scope: self.requirements.scope,
            valid_at: self.requirements.valid_at,
            system_at: self.requirements.system_at,
            commit_id: self.requirements.commit_id,
            answerability_question: Some(self.task.answerability_question),
            evidence_source: self.requirements.evidence_source,
            dependency_target: self.requirements.dependency_target,
            dependency_kind: self.requirements.dependency_kind,
            minimum_confidence: self.requirements.minimum_confidence,
            token_budget: self.requirements.token_budget,
        })
    }
}

/// Query task identity and answerability intent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryTask {
    /// Stable task name or identifier.
    pub name: String,
    /// Exact question or intent selected StateCells should answer.
    pub answerability_question: String,
}

impl QueryTask {
    /// Creates a query task.
    pub fn new(name: impl Into<String>, answerability_question: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            answerability_question: answerability_question.into(),
        }
    }
}

/// Deterministic requirements accepted by the first checkout query compiler.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryRequirements {
    /// Optional scope filter.
    pub scope: Option<Scope>,
    /// Optional valid-time filter.
    pub valid_at: Option<DateTime<Utc>>,
    /// Optional system transaction-time filter.
    pub system_at: Option<DateTime<Utc>>,
    /// Optional database commit identifier filter.
    pub commit_id: Option<CommitId>,
    /// Optional exact evidence-source filter.
    pub evidence_source: Option<String>,
    /// Optional dependency target filter.
    pub dependency_target: Option<StateCellId>,
    /// Optional dependency kind filter.
    pub dependency_kind: Option<CellDependencyKind>,
    /// Minimum evidence confidence for included cells.
    pub minimum_confidence: Confidence,
    /// Maximum token budget for the returned slice.
    pub token_budget: i64,
}

impl Default for QueryRequirements {
    fn default() -> Self {
        Self {
            scope: None,
            valid_at: None,
            system_at: None,
            commit_id: None,
            evidence_source: None,
            dependency_target: None,
            dependency_kind: None,
            minimum_confidence: zero_confidence(),
            token_budget: i64::MAX,
        }
    }
}

fn zero_confidence() -> Confidence {
    Confidence::new(0.0).unwrap_or_default()
}

/// Requested checkout materialization shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryReturnShape {
    /// Current full checkout slice shape.
    PackedContextWithMetadata,
    /// Future cell-only projection.
    CellsOnly,
}

/// Requested checkout optimization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryOptimization {
    /// Current deterministic utility-aware ranking.
    DeterministicUtility,
    /// Future cost-only packing.
    TokenCostOnly,
}

/// Query compilation errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryError {
    /// The requested return shape is not supported by this compiler.
    #[error("unsupported return shape: {0:?}")]
    UnsupportedReturnShape(QueryReturnShape),
    /// The requested optimization policy is not supported by this compiler.
    #[error("unsupported optimization: {0:?}")]
    UnsupportedOptimization(QueryOptimization),
}

/// Query envelope encoding and decoding errors.
#[derive(Debug, Error, PartialEq)]
pub enum QueryEnvelopeError {
    /// Query envelope JSON could not be encoded or decoded.
    #[error("query envelope JSON is invalid")]
    InvalidJson,
    /// Query envelope has an unsupported format or version.
    #[error("query envelope is invalid")]
    InvalidEnvelope,
}

/// Encodes a typed query as a versioned JSON envelope.
pub fn encode_query_json(query: ContinuityQuery) -> Result<Vec<u8>, QueryEnvelopeError> {
    serde_json::to_vec(&QueryEnvelope::new(query)).map_err(|_error| QueryEnvelopeError::InvalidJson)
}

/// Decodes a typed query from a versioned JSON envelope.
pub fn decode_query_json(bytes: &[u8]) -> Result<ContinuityQuery, QueryEnvelopeError> {
    let envelope = serde_json::from_slice::<QueryEnvelope>(bytes)
        .map_err(|_error| QueryEnvelopeError::InvalidJson)?;
    envelope.validate()?;
    Ok(envelope.query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use continuitydb_core::{CellDependencyKind, CommitId, Confidence, Scope, StateCellId};

    #[test]
    fn minimal_checkout_query_compiles_task_answerability_and_defaults(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let request = CheckoutQuery::new(task).compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        assert_eq!(request.scope, None);
        assert_eq!(request.valid_at, None);
        assert_eq!(request.system_at, None);
        assert_eq!(request.commit_id, None);
        assert_eq!(request.evidence_source, None);
        assert_eq!(request.dependency_target, None);
        assert_eq!(request.dependency_kind, None);
        assert_eq!(request.minimum_confidence, Confidence::new(0.0)?);
        assert_eq!(request.token_budget, i64::MAX);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_temporal_confidence_scope_and_budget_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let system_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 11, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let commit_id = CommitId::new();
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let requirements = QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            system_at: Some(system_at),
            commit_id: Some(commit_id),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(request.valid_at, Some(valid_at));
        assert_eq!(request.system_at, Some(system_at));
        assert_eq!(request.commit_id, Some(commit_id));
        assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
        assert_eq!(request.token_budget, 1200);
        Ok(())
    }

    #[test]
    fn checkout_query_compiles_evidence_and_dependency_requirements(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dependency_target = StateCellId::new();
        let task = QueryTask::new("audit-risk", "what risks depend on this evidence?");
        let requirements = QueryRequirements {
            evidence_source: Some("source:incident-review".to_string()),
            dependency_target: Some(dependency_target),
            dependency_kind: Some(CellDependencyKind::DependsOn),
            ..QueryRequirements::default()
        };

        let request = CheckoutQuery::new(task)
            .with_requirements(requirements)
            .compile_checkout()?;

        assert_eq!(
            request.evidence_source.as_deref(),
            Some("source:incident-review")
        );
        assert_eq!(request.dependency_target, Some(dependency_target));
        assert_eq!(request.dependency_kind, Some(CellDependencyKind::DependsOn));
        Ok(())
    }

    #[test]
    fn continuity_query_delegates_checkout_compilation() -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        )));

        let request = query.compile_checkout()?;

        assert_eq!(
            request.answerability_question.as_deref(),
            Some("what is the release status?")
        );
        Ok(())
    }

    #[test]
    fn unsupported_return_shape_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let result = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly)
        .compile_checkout();

        let error = result
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported return shape should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly)
        );
        Ok(())
    }

    #[test]
    fn unsupported_optimization_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
        let result = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_optimization(QueryOptimization::TokenCostOnly)
        .compile_checkout();

        let error = result
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported optimization should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedOptimization(QueryOptimization::TokenCostOnly)
        );
        Ok(())
    }

    #[test]
    fn checkout_query_round_trips_json_and_compiles() -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let task = QueryTask::new("release-readiness", "what is the release status?");
        let query = CheckoutQuery::new(task).with_requirements(QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        });

        let encoded = serde_json::to_vec(&query)?;
        let decoded: CheckoutQuery = serde_json::from_slice(&encoded)?;
        let request = decoded.compile_checkout()?;

        assert_eq!(
            request.scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(request.valid_at, Some(valid_at));
        assert_eq!(request.minimum_confidence, Confidence::new(0.7)?);
        assert_eq!(request.token_budget, 1200);
        Ok(())
    }

    #[test]
    fn continuity_query_serializes_with_snake_case_checkout_tag(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        )));

        let value = serde_json::to_value(&query)?;
        let decoded: ContinuityQuery = serde_json::from_value(value.clone())?;

        assert!(value.get("checkout").is_some());
        assert_eq!(decoded, query);
        Ok(())
    }

    #[test]
    fn unsupported_query_semantics_survive_json_round_trip(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = CheckoutQuery::new(QueryTask::new(
            "release-readiness",
            "what is the release status?",
        ))
        .with_return_shape(QueryReturnShape::CellsOnly)
        .with_optimization(QueryOptimization::TokenCostOnly);

        let value = serde_json::to_value(&query)?;
        assert_eq!(value["return_shape"].as_str(), Some("cells_only"));
        assert_eq!(value["optimization"].as_str(), Some("token_cost_only"));

        let decoded: CheckoutQuery = serde_json::from_value(value)?;
        let error = decoded
            .compile_checkout()
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported return shape should fail"))?;

        assert_eq!(
            error,
            QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly)
        );
        Ok(())
    }

    #[test]
    fn query_envelope_encodes_format_version_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
            "stored-facts",
            "what is stored?",
        )));

        let encoded = encode_query_json(query.clone())?;
        let value: serde_json::Value = serde_json::from_slice(&encoded)?;

        assert_eq!(value["format"].as_str(), Some(QUERY_ENVELOPE_FORMAT));
        assert_eq!(
            value["version"].as_u64(),
            Some(QUERY_ENVELOPE_FORMAT_VERSION as u64)
        );
        assert!(value["query"]["checkout"].is_object());
        assert_eq!(decode_query_json(&encoded)?, query);
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_unsupported_format() -> Result<(), Box<dyn std::error::Error>> {
        let envelope = QueryEnvelope {
            format: "continuitydb.other".to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what is stored?",
            ))),
        };
        let encoded = serde_json::to_vec(&envelope)?;

        assert_eq!(
            decode_query_json(&encoded),
            Err(QueryEnvelopeError::InvalidEnvelope)
        );
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_unsupported_version() -> Result<(), Box<dyn std::error::Error>> {
        let envelope = QueryEnvelope {
            format: QUERY_ENVELOPE_FORMAT.to_string(),
            version: QUERY_ENVELOPE_FORMAT_VERSION + 1,
            query: ContinuityQuery::Checkout(CheckoutQuery::new(QueryTask::new(
                "stored-facts",
                "what is stored?",
            ))),
        };
        let encoded = serde_json::to_vec(&envelope)?;

        assert_eq!(
            decode_query_json(&encoded),
            Err(QueryEnvelopeError::InvalidEnvelope)
        );
        Ok(())
    }

    #[test]
    fn query_envelope_rejects_malformed_json() {
        assert_eq!(
            decode_query_json(b"{not valid json}\n"),
            Err(QueryEnvelopeError::InvalidJson)
        );
    }

    #[test]
    fn checkout_query_accessors_expose_typed_semantics() -> Result<(), Box<dyn std::error::Error>> {
        let valid_at = Utc
            .with_ymd_and_hms(2026, 5, 20, 10, 0, 0)
            .single()
            .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?;
        let requirements = QueryRequirements {
            scope: Some(Scope::Project("continuitydb".to_string())),
            valid_at: Some(valid_at),
            minimum_confidence: Confidence::new(0.7)?,
            token_budget: 1200,
            ..QueryRequirements::default()
        };
        let query = CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
            .with_requirements(requirements.clone())
            .with_return_shape(QueryReturnShape::CellsOnly)
            .with_optimization(QueryOptimization::TokenCostOnly);

        assert_eq!(query.task().name, "stored-facts");
        assert_eq!(query.task().answerability_question, "what is stored?");
        assert_eq!(query.requirements(), &requirements);
        assert_eq!(query.return_shape(), QueryReturnShape::CellsOnly);
        assert_eq!(query.optimization(), QueryOptimization::TokenCostOnly);
        Ok(())
    }

    #[test]
    fn decoded_query_envelope_can_be_inspected_before_compile(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let query = ContinuityQuery::Checkout(
            CheckoutQuery::new(QueryTask::new("stored-facts", "what is stored?"))
                .with_return_shape(QueryReturnShape::CellsOnly),
        );
        let encoded = encode_query_json(query)?;
        let decoded = decode_query_json(&encoded)?;

        let ContinuityQuery::Checkout(checkout) = decoded;
        assert_eq!(checkout.task().name, "stored-facts");
        assert_eq!(checkout.return_shape(), QueryReturnShape::CellsOnly);
        let error = checkout
            .compile_checkout()
            .err()
            .ok_or_else(|| std::io::Error::other("unsupported return shape should fail"))?;
        assert_eq!(
            error,
            QueryError::UnsupportedReturnShape(QueryReturnShape::CellsOnly)
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_minimal_checkout() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(r#"CHECKOUT "release" ANSWER "what should ship?""#)?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.task().name, "release");
        assert_eq!(checkout.task().answerability_question, "what should ship?");
        assert_eq!(checkout.requirements(), &QueryRequirements::default());
        Ok(())
    }

    #[test]
    fn text_query_parses_checkout_where_constraints() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE scope = project("continuitydb")
  AND min_confidence >= 0.7
  AND token_budget <= 1200
  AND evidence_source = "source:release-notes""#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().scope,
            Some(Scope::Project("continuitydb".to_string()))
        );
        assert_eq!(
            checkout.requirements().minimum_confidence,
            Confidence::new(0.7)?
        );
        assert_eq!(checkout.requirements().token_budget, 1200);
        assert_eq!(
            checkout.requirements().evidence_source.as_deref(),
            Some("source:release-notes")
        );
        Ok(())
    }

    #[test]
    fn text_query_parses_temporal_and_commit_constraints(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let commit_id = CommitId::new();
        let query = parse_query_text(&format!(
            r#"CHECKOUT "release" ANSWER "what should ship?"
WHERE valid_at = "2026-05-20T12:00:00Z"
  AND system_at = "2026-05-20T12:30:00Z"
  AND commit_id = "{commit_id}""#
        ))?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(
            checkout.requirements().valid_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 20, 12, 0, 0)
                    .single()
                    .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
            )
        );
        assert_eq!(
            checkout.requirements().system_at,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 20, 12, 30, 0)
                    .single()
                    .ok_or_else(|| std::io::Error::other("invalid test timestamp"))?
            )
        );
        assert_eq!(checkout.requirements().commit_id, Some(commit_id));
        Ok(())
    }

    #[test]
    fn text_query_keywords_are_case_insensitive() -> Result<(), Box<dyn std::error::Error>> {
        let query = parse_query_text(
            r#"checkout "release" answer "what should ship?" where scope = global"#,
        )?;

        let ContinuityQuery::Checkout(checkout) = query;
        assert_eq!(checkout.requirements().scope, Some(Scope::Global));
        Ok(())
    }

    #[test]
    fn text_query_rejects_invalid_syntax() {
        assert_eq!(
            parse_query_text(r#"CHECKOUT "release" WHERE scope = global"#),
            Err(QueryTextError::InvalidSyntax)
        );
    }

    #[test]
    fn text_query_rejects_invalid_values() {
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE min_confidence >= 1.5"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE token_budget <= -1"#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE valid_at = "not-a-time""#
            ),
            Err(QueryTextError::InvalidValue)
        );
        assert_eq!(
            parse_query_text(
                r#"CHECKOUT "release" ANSWER "what should ship?" WHERE commit_id = "not-a-uuid""#
            ),
            Err(QueryTextError::InvalidValue)
        );
    }
}
