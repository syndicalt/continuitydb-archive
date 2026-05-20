//! Deterministic frontier watch Steward integration.

use chrono::{DateTime, Utc};
use continuitydb_core::StateCellId;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::{ProposalId, StewardAction, StewardError, StewardIdentity, StewardProposal};

/// Signal observed by frontier/watch integration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrontierWatchSignal {
    /// Evidence for a watched cell is stale and needs refresh.
    StaleEvidence,
    /// A cell is uncertain enough and important enough to enter frontier monitoring.
    HighImpactUncertainty,
    /// The watch event does not require Steward work.
    Benign,
}

/// Immutable identifier for a durable frontier subscription.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct FrontierSubscriptionId(Uuid);

impl FrontierSubscriptionId {
    /// Creates a random frontier subscription identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for FrontierSubscriptionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Durable subscription describing which frontier signals to watch for a cell.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrontierSubscription {
    id: FrontierSubscriptionId,
    cell_id: StateCellId,
    signals: Vec<FrontierWatchSignal>,
    citation: String,
    subscribed_at: DateTime<Utc>,
}

impl FrontierSubscription {
    /// Creates a validated frontier subscription.
    pub fn new(
        id: FrontierSubscriptionId,
        cell_id: StateCellId,
        signals: Vec<FrontierWatchSignal>,
        citation: impl Into<String>,
        subscribed_at: DateTime<Utc>,
    ) -> Result<Self, StewardError> {
        let citation = citation.into().trim().to_string();
        if signals.is_empty() || citation.is_empty() {
            return Err(StewardError::EmptyFrontierSubscription);
        }

        Ok(Self {
            id,
            cell_id,
            signals,
            citation,
            subscribed_at,
        })
    }

    /// Returns this subscription's identifier.
    pub fn id(&self) -> FrontierSubscriptionId {
        self.id
    }

    /// Returns the watched StateCell ID.
    pub fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    /// Returns the frontier signals that activate this subscription.
    pub fn signals(&self) -> &[FrontierWatchSignal] {
        &self.signals
    }

    /// Returns the citation supporting this subscription.
    pub fn citation(&self) -> &str {
        &self.citation
    }

    /// Returns when the subscription was created.
    pub fn subscribed_at(&self) -> DateTime<Utc> {
        self.subscribed_at
    }

    /// Returns whether this subscription should receive the watch event.
    pub fn matches_event(&self, event: &FrontierWatchEvent) -> bool {
        self.cell_id == event.cell_id && self.signals.contains(&event.signal)
    }
}

/// Storage contract for durable frontier subscriptions.
pub trait FrontierSubscriptionStore {
    /// Appends a frontier subscription record.
    fn append_subscription(
        &mut self,
        subscription: FrontierSubscription,
    ) -> Result<(), StewardError>;

    /// Lists all frontier subscriptions in insertion order.
    fn list_subscriptions(&self) -> Result<Vec<FrontierSubscription>, StewardError>;

    /// Returns a frontier subscription by ID.
    fn get_subscription(
        &self,
        subscription_id: FrontierSubscriptionId,
    ) -> Result<Option<FrontierSubscription>, StewardError>;
}

/// In-memory frontier subscription store implementation for correctness tests.
#[derive(Default)]
pub struct MemoryFrontierSubscriptionStore {
    subscriptions: Vec<FrontierSubscription>,
}

impl FrontierSubscriptionStore for MemoryFrontierSubscriptionStore {
    fn append_subscription(
        &mut self,
        subscription: FrontierSubscription,
    ) -> Result<(), StewardError> {
        self.subscriptions.push(subscription);
        Ok(())
    }

    fn list_subscriptions(&self) -> Result<Vec<FrontierSubscription>, StewardError> {
        Ok(self.subscriptions.clone())
    }

    fn get_subscription(
        &self,
        subscription_id: FrontierSubscriptionId,
    ) -> Result<Option<FrontierSubscription>, StewardError> {
        Ok(self
            .subscriptions
            .iter()
            .find(|subscription| subscription.id() == subscription_id)
            .cloned())
    }
}

/// JSONL file-backed frontier subscription store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFrontierSubscriptionStore {
    path: PathBuf,
}

impl FileFrontierSubscriptionStore {
    /// Opens a JSONL frontier subscription store at the supplied path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StewardError> {
        let path = path.as_ref().to_path_buf();
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|_error| StewardError::FrontierSubscriptionStoreIo)?;

        Ok(Self { path })
    }

    /// Returns the backing file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read_subscriptions(&self) -> Result<Vec<FrontierSubscription>, StewardError> {
        let file =
            File::open(&self.path).map_err(|_error| StewardError::FrontierSubscriptionStoreIo)?;
        let reader = BufReader::new(file);
        let mut subscriptions = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|_error| StewardError::FrontierSubscriptionStoreIo)?;
            if line.trim().is_empty() {
                continue;
            }
            subscriptions.push(
                serde_json::from_str(&line)
                    .map_err(|_error| StewardError::FrontierSubscriptionStoreCorrupt)?,
            );
        }

        Ok(subscriptions)
    }
}

impl FrontierSubscriptionStore for FileFrontierSubscriptionStore {
    fn append_subscription(
        &mut self,
        subscription: FrontierSubscription,
    ) -> Result<(), StewardError> {
        let encoded = serde_json::to_string(&subscription)
            .map_err(|_error| StewardError::FrontierSubscriptionStoreCorrupt)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_error| StewardError::FrontierSubscriptionStoreIo)?;

        file.write_all(encoded.as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .map_err(|_error| StewardError::FrontierSubscriptionStoreIo)
    }

    fn list_subscriptions(&self) -> Result<Vec<FrontierSubscription>, StewardError> {
        self.read_subscriptions()
    }

    fn get_subscription(
        &self,
        subscription_id: FrontierSubscriptionId,
    ) -> Result<Option<FrontierSubscription>, StewardError> {
        Ok(self
            .read_subscriptions()?
            .into_iter()
            .find(|subscription| subscription.id() == subscription_id))
    }
}

/// Evidence-backed frontier watch event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrontierWatchEvent {
    cell_id: StateCellId,
    signal: FrontierWatchSignal,
    citation: String,
    observed_at: DateTime<Utc>,
}

impl FrontierWatchEvent {
    /// Creates a frontier watch event.
    pub fn new(
        cell_id: StateCellId,
        signal: FrontierWatchSignal,
        citation: impl Into<String>,
        observed_at: DateTime<Utc>,
    ) -> Self {
        Self {
            cell_id,
            signal,
            citation: citation.into(),
            observed_at,
        }
    }

    /// Returns the watched cell ID.
    pub fn cell_id(&self) -> StateCellId {
        self.cell_id
    }

    /// Returns the frontier watch signal.
    pub fn signal(&self) -> FrontierWatchSignal {
        self.signal
    }

    /// Returns the event citation locator.
    pub fn citation(&self) -> &str {
        &self.citation
    }

    /// Returns when the event was observed.
    pub fn observed_at(&self) -> DateTime<Utc> {
        self.observed_at
    }
}

/// Deterministic Steward for frontier/watch events.
#[derive(Clone, Debug)]
pub struct FrontierSteward {
    identity: StewardIdentity,
}

impl FrontierSteward {
    /// Creates a frontier Steward with a stable identity.
    pub fn new(identity: StewardIdentity) -> Self {
        Self { identity }
    }

    /// Converts frontier watch events into validated Steward proposals.
    pub fn propose(
        &self,
        events: Vec<FrontierWatchEvent>,
        created_at: DateTime<Utc>,
    ) -> Result<Vec<StewardProposal>, StewardError> {
        let mut proposals = Vec::new();
        for event in events {
            if let Some(proposal) = self.proposal_for_event(event, created_at) {
                proposals.push(proposal?);
            }
        }
        Ok(proposals)
    }

    fn proposal_for_event(
        &self,
        event: FrontierWatchEvent,
        created_at: DateTime<Utc>,
    ) -> Option<Result<StewardProposal, StewardError>> {
        match event.signal {
            FrontierWatchSignal::StaleEvidence => Some(StewardProposal::new(
                ProposalId::new(),
                self.identity.clone(),
                StewardAction::RequestVerification {
                    cell_id: Some(event.cell_id),
                    request: "Refresh stale evidence for frontier cell.".to_string(),
                },
                "Watched frontier evidence is stale and requires verification.",
                vec![event.citation],
                created_at,
            )),
            FrontierWatchSignal::HighImpactUncertainty => Some(StewardProposal::new(
                ProposalId::new(),
                self.identity.clone(),
                StewardAction::MarkFrontier {
                    cell_id: event.cell_id,
                },
                "Watched cell has high-impact uncertainty.",
                vec![event.citation],
                created_at,
            )),
            FrontierWatchSignal::Benign => None,
        }
    }
}
