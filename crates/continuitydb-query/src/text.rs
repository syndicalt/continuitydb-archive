//! Text parser for ContinuityDB queries.

use chrono::{DateTime, Utc};
use continuitydb_core::{CommitId, Confidence, Scope};
use thiserror::Error;

use crate::{CheckoutQuery, ContinuityQuery, QueryRequirements, QueryTask};

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
        if self.consume_keyword("where") {
            self.parse_constraint(&mut requirements)?;
            while self.consume_keyword("and") {
                self.parse_constraint(&mut requirements)?;
            }
        }

        if self.peek().is_some() {
            return Err(QueryTextError::InvalidSyntax);
        }

        Ok(ContinuityQuery::Checkout(
            CheckoutQuery::new(QueryTask::new(task_name, question)).with_requirements(requirements),
        ))
    }

    fn parse_constraint(
        &mut self,
        requirements: &mut QueryRequirements,
    ) -> Result<(), QueryTextError> {
        let field = self.expect_ident()?;
        match field.to_ascii_lowercase().as_str() {
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
            "min_confidence" => {
                self.expect_token(Token::Gte)?;
                let value = self
                    .expect_number()?
                    .parse::<f32>()
                    .map_err(|_error| QueryTextError::InvalidValue)?;
                requirements.minimum_confidence =
                    Confidence::new(value).map_err(|_error| QueryTextError::InvalidValue)?;
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
