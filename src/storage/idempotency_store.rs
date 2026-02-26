//! Idempotency tracking for fact injection.
//!
//! The IdempotencyStore tracks external_id claims to ensure fact events are
//! only persisted once. When a fact is injected with an external_id, the store
//! records the claim. Subsequent injections with the same external_id return
//! the original claim without re-persisting.

use async_trait::async_trait;
use uuid::Uuid;

use super::Result;

/// Record of a claimed external_id.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    /// Domain the fact was injected into.
    pub domain: String,
    /// Edition (timeline) for the aggregate.
    pub edition: String,
    /// Aggregate root UUID.
    pub root: Uuid,
    /// The external_id that was claimed.
    pub external_id: String,
    /// First sequence number of the injected events.
    pub first_sequence: u32,
    /// Last sequence number of the injected events.
    pub last_sequence: u32,
    /// When the claim was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Store for tracking idempotency of fact injections.
///
/// Each (domain, edition, root, external_id) combination can only be claimed once.
/// The store tracks the sequence range of the originally injected events so they
/// can be retrieved on duplicate requests.
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Try to claim an external_id for a fact injection.
    ///
    /// If the external_id hasn't been claimed, records the claim and returns `Ok(None)`.
    /// If already claimed, returns `Ok(Some(record))` with the existing claim.
    ///
    /// This operation must be atomic - only one caller can successfully claim.
    async fn try_claim(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        external_id: &str,
        first_sequence: u32,
        last_sequence: u32,
    ) -> Result<Option<IdempotencyRecord>>;

    /// Check if an external_id has been claimed.
    ///
    /// Returns the claim record if it exists, None otherwise.
    async fn get(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        external_id: &str,
    ) -> Result<Option<IdempotencyRecord>>;
}
