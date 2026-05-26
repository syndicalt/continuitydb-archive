//! Text parser for ContinuityDB queries.

use chrono::{DateTime, Utc};
use continuitydb_core::{
    ActivationState, CellDependencyKind, CommitId, Confidence, ContextCompilerPolicy,
    ContextGapKind, ContextPacketSelectionReason, ContextPacketStrategy, ContextProfile,
    EpistemicAction, EpistemicActionReason, InvalidationConditionKind, LifecycleStage,
    MemoryProjectionKind, PromotionPolicy, RetentionPolicy, RevisionLinkKind, Scope,
    SemanticAnchor, StateCellId, UsePolicy,
};
use thiserror::Error;

use crate::{CheckoutQuery, ContinuityQuery, QueryRequirements, QueryReturnShape, QueryTask};

/// Text query parsing errors.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum QueryTextError {
    /// Input does not match the supported text query grammar.
    #[error("query text syntax is invalid")]
    InvalidSyntax,
    /// Input contains a syntactically valid but unsupported value.
    #[error("query text value is invalid")]
    InvalidValue,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    String(String),
    Number(String),
    Eq,
    Gte,
    Lte,
    LParen,
    RParen,
}

/// Parses a strict text query into the typed Continuity Query AST.
pub fn parse_query_text(input: &str) -> Result<ContinuityQuery, QueryTextError> {
    Parser::new(Lexer::new(input).lex()?).parse_query()
}

struct Lexer<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, offset: 0 }
    }

    fn lex(mut self) -> Result<Vec<Token>, QueryTextError> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.advance_char();
                continue;
            }

            match ch {
                '"' => tokens.push(Token::String(self.lex_string()?)),
                '(' => {
                    self.advance_char();
                    tokens.push(Token::LParen);
                }
                ')' => {
                    self.advance_char();
                    tokens.push(Token::RParen);
                }
                '=' => {
                    self.advance_char();
                    tokens.push(Token::Eq);
                }
                '>' => {
                    self.advance_char();
                    if self.consume_char('=') {
                        tokens.push(Token::Gte);
                    } else {
                        return Err(QueryTextError::InvalidSyntax);
                    }
                }
                '<' => {
                    self.advance_char();
                    if self.consume_char('=') {
                        tokens.push(Token::Lte);
                    } else {
                        return Err(QueryTextError::InvalidSyntax);
                    }
                }
                '-' | '0'..='9' => tokens.push(Token::Number(self.lex_number())),
                _ if is_ident_start(ch) => tokens.push(Token::Ident(self.lex_ident())),
                _ => return Err(QueryTextError::InvalidSyntax),
            }
        }
        Ok(tokens)
    }

    fn lex_string(&mut self) -> Result<String, QueryTextError> {
        self.advance_char();
        let mut value = String::new();
        while let Some(ch) = self.peek_char() {
            self.advance_char();
            if ch == '"' {
                return Ok(value);
            }
            value.push(ch);
        }
        Err(QueryTextError::InvalidSyntax)
    }

    fn lex_number(&mut self) -> String {
        let mut value = String::new();
        while let Some(ch) = self.peek_char() {
            if ch == '-' || ch == '.' || ch.is_ascii_digit() {
                value.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }
        value
    }

    fn lex_ident(&mut self) -> String {
        let mut value = String::new();
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) {
                value.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }
        value
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.offset += ch.len_utf8();
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance_char();
            true
        } else {
            false
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_query(mut self) -> Result<ContinuityQuery, QueryTextError> {
        self.expect_keyword("checkout")?;
        let task_name = self.expect_string()?;
        self.expect_keyword("answer")?;
        let question = self.expect_string()?;

        let mut requirements = QueryRequirements::default();
        let mut compiler_policy = ContextCompilerPolicy::Automatic;
        let mut compiler_intent = None;
        if self.consume_keyword("where") {
            self.parse_constraint(
                &mut requirements,
                &mut compiler_policy,
                &mut compiler_intent,
            )?;
            while self.consume_keyword("and") {
                self.parse_constraint(
                    &mut requirements,
                    &mut compiler_policy,
                    &mut compiler_intent,
                )?;
            }
        }
        let mut return_shape = QueryReturnShape::PackedContextWithMetadata;
        if self.consume_keyword("return") {
            return_shape = self.parse_return_shape()?;
        }

        if self.peek().is_some() {
            return Err(QueryTextError::InvalidSyntax);
        }

        Ok(ContinuityQuery::Checkout(
            CheckoutQuery::new(QueryTask::new(task_name, question))
                .with_requirements(requirements)
                .with_compiler_policy(compiler_policy)
                .with_optional_compiler_intent(compiler_intent)
                .with_return_shape(return_shape),
        ))
    }

    fn parse_constraint(
        &mut self,
        requirements: &mut QueryRequirements,
        compiler_policy: &mut ContextCompilerPolicy,
        compiler_intent: &mut Option<String>,
    ) -> Result<(), QueryTextError> {
        let field = self.expect_ident()?;
        match field.to_ascii_lowercase().as_str() {
            "semantic_anchor" => {
                self.expect_token(Token::Eq)?;
                requirements.semantic_anchor = Some(SemanticAnchor::new(self.expect_string()?));
            }
            "scope" => {
                self.expect_token(Token::Eq)?;
                requirements.scope = Some(self.parse_scope()?);
            }
            "valid_at" => {
                self.expect_token(Token::Eq)?;
                requirements.valid_at = Some(self.parse_datetime()?);
            }
            "system_at" => {
                self.expect_token(Token::Eq)?;
                requirements.system_at = Some(self.parse_datetime()?);
            }
            "commit_id" => {
                self.expect_token(Token::Eq)?;
                requirements.commit_id = Some(self.parse_commit_id()?);
            }
            "activation" => {
                self.expect_token(Token::Eq)?;
                requirements.activation = Some(self.parse_activation()?);
            }
            "lifecycle_stage" => {
                self.expect_token(Token::Eq)?;
                requirements.lifecycle_stage = Some(self.parse_lifecycle_stage()?);
            }
            "projection_kind" => {
                self.expect_token(Token::Eq)?;
                requirements.projection_kind = Some(self.parse_projection_kind()?);
            }
            "context_gap_kind" => {
                self.expect_token(Token::Eq)?;
                requirements.context_gap_kind = Some(self.parse_context_gap_kind()?);
            }
            "invalidation_condition_kind" => {
                self.expect_token(Token::Eq)?;
                requirements.invalidation_condition_kind =
                    Some(self.parse_invalidation_condition_kind()?);
            }
            "dependency_target" => {
                self.expect_token(Token::Eq)?;
                requirements.dependency_target = Some(self.parse_state_cell_id()?);
            }
            "dependency_kind" => {
                self.expect_token(Token::Eq)?;
                requirements.dependency_kind = Some(self.parse_dependency_kind()?);
            }
            "revision_related_cell" => {
                self.expect_token(Token::Eq)?;
                requirements.revision_related_cell = Some(self.parse_state_cell_id()?);
            }
            "revision_link_kind" => {
                self.expect_token(Token::Eq)?;
                requirements.revision_link_kind = Some(self.parse_revision_link_kind()?);
            }
            "context_profile" => {
                self.expect_token(Token::Eq)?;
                requirements.context_profile = self.parse_context_profile()?;
            }
            "compiler_policy" => {
                self.expect_token(Token::Eq)?;
                *compiler_policy = self.parse_compiler_policy()?;
            }
            "compiler_intent" => {
                self.expect_token(Token::Eq)?;
                *compiler_intent = Some(self.expect_string()?);
            }
            "min_confidence" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_confidence =
                    Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?;
            }
            "min_uncertainty" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_uncertainty =
                    Some(Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?);
            }
            "min_surprise_bits" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if !value.is_finite() || value < 0.0 {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.minimum_surprise_bits = Some(value);
            }
            "min_probability_delta" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if !(0.0..=1.0).contains(&value) {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.minimum_probability_delta = Some(value);
            }
            "min_salience" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if !(0.0..=1.0).contains(&value) {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.minimum_salience = Some(value);
            }
            "min_context_affordance" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if !(0.0..=1.0).contains(&value) {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.minimum_context_affordance = Some(value);
            }
            "min_epistemic_pressure" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if !(0.0..=1.0).contains(&value) {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.minimum_epistemic_pressure = Some(value);
            }
            "min_context_gap_priority" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_context_gap_priority =
                    Some(Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?);
            }
            "min_invalidation_priority" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_invalidation_priority =
                    Some(Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?);
            }
            "epistemic_action" => {
                self.expect_token(Token::Eq)?;
                requirements.epistemic_action = Some(self.parse_epistemic_action()?);
            }
            "epistemic_action_reason" => {
                self.expect_token(Token::Eq)?;
                requirements.epistemic_action_reason = Some(self.parse_epistemic_action_reason()?);
            }
            "selection_reason" => {
                self.expect_token(Token::Eq)?;
                requirements.selection_reason = Some(self.parse_selection_reason()?);
            }
            "trajectory_memory_strategy" => {
                self.expect_token(Token::Eq)?;
                requirements.trajectory_memory_strategy =
                    Some(self.parse_context_packet_strategy()?);
            }
            "min_trajectory_memory_confidence" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_trajectory_memory_confidence =
                    Some(Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?);
            }
            "retention_policy" => {
                self.expect_token(Token::Eq)?;
                requirements.retention_policy = Some(self.parse_retention_policy()?);
            }
            "use_policy" => {
                self.expect_token(Token::Eq)?;
                requirements.use_policy = Some(self.parse_use_policy()?);
            }
            "promotion_policy" => {
                self.expect_token(Token::Eq)?;
                requirements.promotion_policy = Some(self.parse_promotion_policy()?);
            }
            "token_budget" => {
                self.expect_token(Token::Lte)?;
                let value = self
                    .expect_number()?
                    .parse::<i64>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                if value < 0 {
                    return Err(QueryTextError::InvalidValue);
                }
                requirements.token_budget = value;
            }
            "evidence_source" => {
                self.expect_token(Token::Eq)?;
                requirements.evidence_source = Some(self.expect_string()?);
            }
            _ => return Err(QueryTextError::InvalidSyntax),
        }
        Ok(())
    }

    fn parse_datetime(&mut self) -> Result<DateTime<Utc>, QueryTextError> {
        DateTime::parse_from_rfc3339(&self.expect_string()?)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_error| QueryTextError::InvalidValue)
    }

    fn parse_commit_id(&mut self) -> Result<CommitId, QueryTextError> {
        self.expect_string()?
            .parse::<CommitId>()
            .map_err(|_error| QueryTextError::InvalidValue)
    }

    fn parse_activation(&mut self) -> Result<ActivationState, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "dormant" => Ok(ActivationState::Dormant),
            "active" => Ok(ActivationState::Active),
            "frontier" => Ok(ActivationState::Frontier),
            "retired" => Ok(ActivationState::Retired),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_state_cell_id(&mut self) -> Result<StateCellId, QueryTextError> {
        self.expect_string()?
            .parse::<StateCellId>()
            .map_err(|_error| QueryTextError::InvalidValue)
    }

    fn parse_dependency_kind(&mut self) -> Result<CellDependencyKind, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "depends_on" => Ok(CellDependencyKind::DependsOn),
            "caused_by" => Ok(CellDependencyKind::CausedBy),
            "supports" => Ok(CellDependencyKind::Supports),
            "derived_from" => Ok(CellDependencyKind::DerivedFrom),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_revision_link_kind(&mut self) -> Result<RevisionLinkKind, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "predecessor" => Ok(RevisionLinkKind::Predecessor),
            "supersedes" => Ok(RevisionLinkKind::Supersedes),
            "conflicts_with" => Ok(RevisionLinkKind::ConflictsWith),
            "derives_from" => Ok(RevisionLinkKind::DerivesFrom),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_context_profile(&mut self) -> Result<ContextProfile, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "debugging" => Ok(ContextProfile::Debugging),
            "planning" => Ok(ContextProfile::Planning),
            "execution" => Ok(ContextProfile::Execution),
            "audit" => Ok(ContextProfile::Audit),
            "reflection" => Ok(ContextProfile::Reflection),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_compiler_policy(&mut self) -> Result<ContextCompilerPolicy, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "raw_baseline" => Ok(ContextCompilerPolicy::RawBaseline),
            "automatic" => Ok(ContextCompilerPolicy::Automatic),
            "model_assisted" => Ok(ContextCompilerPolicy::ModelAssisted),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_lifecycle_stage(&mut self) -> Result<LifecycleStage, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "observed" => Ok(LifecycleStage::Observed),
            "believed" => Ok(LifecycleStage::Believed),
            "contradicted" => Ok(LifecycleStage::Contradicted),
            "superseded" => Ok(LifecycleStage::Superseded),
            "consolidated" => Ok(LifecycleStage::Consolidated),
            "abstracted" => Ok(LifecycleStage::Abstracted),
            "operationalized" => Ok(LifecycleStage::Operationalized),
            "decayed" => Ok(LifecycleStage::Decayed),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_projection_kind(&mut self) -> Result<MemoryProjectionKind, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "episodic" => Ok(MemoryProjectionKind::Episodic),
            "semantic" => Ok(MemoryProjectionKind::Semantic),
            "procedural" => Ok(MemoryProjectionKind::Procedural),
            "policy" => Ok(MemoryProjectionKind::Policy),
            "uncertainty" => Ok(MemoryProjectionKind::Uncertainty),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_context_gap_kind(&mut self) -> Result<ContextGapKind, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "missing_evidence" => Ok(ContextGapKind::MissingEvidence),
            "missing_decision" => Ok(ContextGapKind::MissingDecision),
            "missing_constraint" => Ok(ContextGapKind::MissingConstraint),
            "missing_dependency" => Ok(ContextGapKind::MissingDependency),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_invalidation_condition_kind(
        &mut self,
    ) -> Result<InvalidationConditionKind, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "contradictory_evidence" => Ok(InvalidationConditionKind::ContradictoryEvidence),
            "boundary_violation" => Ok(InvalidationConditionKind::BoundaryViolation),
            "temporal_expiry" => Ok(InvalidationConditionKind::TemporalExpiry),
            "dependency_invalidated" => Ok(InvalidationConditionKind::DependencyInvalidated),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_epistemic_action(&mut self) -> Result<EpistemicAction, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "use" => Ok(EpistemicAction::Use),
            "hedge" => Ok(EpistemicAction::Hedge),
            "verify" => Ok(EpistemicAction::Verify),
            "revise" => Ok(EpistemicAction::Revise),
            "scavenge" => Ok(EpistemicAction::Scavenge),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_epistemic_action_reason(&mut self) -> Result<EpistemicActionReason, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "high_uncertainty" => Ok(EpistemicActionReason::HighUncertainty),
            "high_surprise" => Ok(EpistemicActionReason::HighSurprise),
            "moderate_uncertainty" => Ok(EpistemicActionReason::ModerateUncertainty),
            "low_evidence_confidence" => Ok(EpistemicActionReason::LowEvidenceConfidence),
            "miscalibrated_confidence" => Ok(EpistemicActionReason::MiscalibratedConfidence),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_selection_reason(&mut self) -> Result<ContextPacketSelectionReason, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "evidence_confidence" => Ok(ContextPacketSelectionReason::EvidenceConfidence),
            "utility_feedback" => Ok(ContextPacketSelectionReason::UtilityFeedback),
            "lifecycle_stage" => Ok(ContextPacketSelectionReason::LifecycleStage),
            "projection_profile" => Ok(ContextPacketSelectionReason::ProjectionProfile),
            "native_uncertainty" => Ok(ContextPacketSelectionReason::NativeUncertainty),
            "epistemic_calibration" => Ok(ContextPacketSelectionReason::EpistemicCalibration),
            "context_affordance" => Ok(ContextPacketSelectionReason::ContextAffordance),
            "context_gap" => Ok(ContextPacketSelectionReason::ContextGap),
            "invalidation_condition" => Ok(ContextPacketSelectionReason::InvalidationCondition),
            "trajectory_memory" => Ok(ContextPacketSelectionReason::TrajectoryMemory),
            "lifecycle_policy" => Ok(ContextPacketSelectionReason::LifecyclePolicy),
            "attention_signal" => Ok(ContextPacketSelectionReason::AttentionSignal),
            "answerability" => Ok(ContextPacketSelectionReason::Answerability),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_context_packet_strategy(&mut self) -> Result<ContextPacketStrategy, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "raw_projection" => Ok(ContextPacketStrategy::RawProjection),
            "operational_brief" => Ok(ContextPacketStrategy::OperationalBrief),
            "revision_capsule" => Ok(ContextPacketStrategy::RevisionCapsule),
            "uncertainty_brief" => Ok(ContextPacketStrategy::UncertaintyBrief),
            "scavenging_brief" => Ok(ContextPacketStrategy::ScavengingBrief),
            "falsification_brief" => Ok(ContextPacketStrategy::FalsificationBrief),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_retention_policy(&mut self) -> Result<RetentionPolicy, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "persistent" => Ok(RetentionPolicy::Persistent),
            "decay_unless_reinforced" => Ok(RetentionPolicy::DecayUnlessReinforced),
            "ephemeral" => Ok(RetentionPolicy::Ephemeral),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_use_policy(&mut self) -> Result<UsePolicy, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "use_directly" => Ok(UsePolicy::UseDirectly),
            "hedge_before_use" => Ok(UsePolicy::HedgeBeforeUse),
            "verify_before_use" => Ok(UsePolicy::VerifyBeforeUse),
            "do_not_use_for_answer" => Ok(UsePolicy::DoNotUseForAnswer),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_promotion_policy(&mut self) -> Result<PromotionPolicy, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "manual" => Ok(PromotionPolicy::Manual),
            "evidence_count" => {
                self.expect_token(Token::LParen)?;
                let value = self
                    .expect_number()?
                    .parse::<usize>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                self.expect_token(Token::RParen)?;
                Ok(PromotionPolicy::EvidenceCount(value))
            }
            "confidence_threshold" => {
                self.expect_token(Token::LParen)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                self.expect_token(Token::RParen)?;
                Ok(PromotionPolicy::ConfidenceThreshold(
                    Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?,
                ))
            }
            "repeated_observation" => {
                self.expect_token(Token::LParen)?;
                let value = self
                    .expect_number()?
                    .parse::<usize>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                self.expect_token(Token::RParen)?;
                Ok(PromotionPolicy::RepeatedObservation(value))
            }
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_return_shape(&mut self) -> Result<QueryReturnShape, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "packed_context_with_metadata" => Ok(QueryReturnShape::PackedContextWithMetadata),
            "summary_only" => Ok(QueryReturnShape::SummaryOnly),
            "cells_only" => Ok(QueryReturnShape::CellsOnly),
            "context_packets_only" => Ok(QueryReturnShape::ContextPacketsOnly),
            _ => Err(QueryTextError::InvalidValue),
        }
    }

    fn parse_scope(&mut self) -> Result<Scope, QueryTextError> {
        let ident = self.expect_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "global" => Ok(Scope::Global),
            "project" => self.parse_named_scope(Scope::Project),
            "team" => self.parse_named_scope(Scope::Team),
            "org" => self.parse_named_scope(Scope::Organization),
            "personal" => self.parse_named_scope(Scope::Personal),
            "task" => self.parse_named_scope(Scope::Task),
            _ => Err(QueryTextError::InvalidSyntax),
        }
    }

    fn parse_named_scope<F>(&mut self, constructor: F) -> Result<Scope, QueryTextError>
    where
        F: FnOnce(String) -> Scope,
    {
        self.expect_token(Token::LParen)?;
        let value = self.expect_string()?;
        self.expect_token(Token::RParen)?;
        Ok(constructor(value))
    }

    fn expect_keyword(&mut self, expected: &str) -> Result<(), QueryTextError> {
        if self.consume_keyword(expected) {
            Ok(())
        } else {
            Err(QueryTextError::InvalidSyntax)
        }
    }

    fn consume_keyword(&mut self, expected: &str) -> bool {
        match self.peek() {
            Some(Token::Ident(value)) if value.eq_ignore_ascii_case(expected) => {
                self.position += 1;
                true
            }
            _ => false,
        }
    }

    fn expect_ident(&mut self) -> Result<String, QueryTextError> {
        match self.next() {
            Some(Token::Ident(value)) => Ok(value),
            _ => Err(QueryTextError::InvalidSyntax),
        }
    }

    fn expect_string(&mut self) -> Result<String, QueryTextError> {
        match self.next() {
            Some(Token::String(value)) => Ok(value),
            _ => Err(QueryTextError::InvalidSyntax),
        }
    }

    fn expect_number(&mut self) -> Result<String, QueryTextError> {
        match self.next() {
            Some(Token::Number(value)) => Ok(value),
            _ => Err(QueryTextError::InvalidSyntax),
        }
    }

    fn expect_token(&mut self, expected: Token) -> Result<(), QueryTextError> {
        match self.next() {
            Some(token) if token == expected => Ok(()),
            _ => Err(QueryTextError::InvalidSyntax),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        if token.is_some() {
            self.position += 1;
        }
        token
    }
}
