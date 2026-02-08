//! Hybrid destination fetcher for process manager sidecars.
//!
//! Routes PM domain queries to local storage, all others to gRPC.
//! Solves the chicken-and-egg problem: PM sidecar needs to query its own
//! domain's state, but there's no aggregate service for the PM domain.
//! Uses `EventBookRepository` to properly load snapshots and only subsequent events.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::proto::{Cover, EventBook};
use crate::proto_ext::CoverExt;
use crate::repository::EventBookRepository;
use crate::storage::{EventStore, SnapshotStore};

use super::DestinationFetcher;

/// Fetches from local storage for one domain, delegates others to a remote fetcher.
///
/// Used by the process manager sidecar to access its own state directly while
/// routing other domain queries to remote aggregate sidecars via gRPC.
pub struct HybridDestinationFetcher {
    local_domain: String,
    local_event_store: Arc<dyn EventStore>,
    local_snapshot_store: Arc<dyn SnapshotStore>,
    remote: Arc<dyn DestinationFetcher>,
}

impl HybridDestinationFetcher {
    /// Create with the local domain name, stores, and remote fetcher for other domains.
    pub fn new(
        local_domain: String,
        local_event_store: Arc<dyn EventStore>,
        local_snapshot_store: Arc<dyn SnapshotStore>,
        remote: Arc<dyn DestinationFetcher>,
    ) -> Self {
        Self {
            local_domain,
            local_event_store,
            local_snapshot_store,
            remote,
        }
    }
}

#[async_trait]
impl DestinationFetcher for HybridDestinationFetcher {
    async fn fetch(&self, cover: &Cover) -> Option<EventBook> {
        if cover.domain == self.local_domain {
            let root = cover
                .root
                .as_ref()
                .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())?;
            let edition = cover.edition();

            // Use EventBookRepository to properly load snapshot + subsequent events
            let repo = EventBookRepository::new(
                self.local_event_store.clone(),
                self.local_snapshot_store.clone(),
            );

            match repo.get(&self.local_domain, edition, root).await {
                Ok(mut book) => {
                    // Preserve the correlation_id from the input cover
                    if let Some(ref mut book_cover) = book.cover {
                        book_cover.correlation_id = cover.correlation_id.clone();
                    }
                    Some(book)
                }
                Err(e) => {
                    warn!(
                        domain = %self.local_domain,
                        error = %e,
                        "Failed to fetch local domain EventBook"
                    );
                    None
                }
            }
        } else {
            self.remote.fetch(cover).await
        }
    }

    async fn fetch_by_correlation(&self, domain: &str, correlation_id: &str) -> Option<EventBook> {
        if domain == self.local_domain {
            debug!(
                domain = %domain,
                correlation_id = %correlation_id,
                "Fetching PM state from local store"
            );

            // First find the aggregate by correlation_id
            let books = match self.local_event_store.get_by_correlation(correlation_id).await {
                Ok(books) => books,
                Err(e) => {
                    warn!(
                        domain = %domain,
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to fetch local domain state by correlation"
                    );
                    return None;
                }
            };

            // Find the first book matching this domain
            let book = books.into_iter().find(|b| {
                b.cover.as_ref().is_some_and(|c| c.domain == self.local_domain)
            })?;

            // Re-fetch using EventBookRepository to get snapshot-optimized version
            let cover = book.cover.as_ref()?;
            let edition = cover.edition.as_ref().map(|e| e.name.as_str()).unwrap_or("main");
            let root_uuid = cover.root.as_ref()
                .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())?;

            let repo = EventBookRepository::new(
                self.local_event_store.clone(),
                self.local_snapshot_store.clone(),
            );

            match repo.get(domain, edition, root_uuid).await {
                Ok(mut fetched_book) => {
                    // Preserve the correlation_id
                    if let Some(ref mut fetched_cover) = fetched_book.cover {
                        fetched_cover.correlation_id = correlation_id.to_string();
                    }
                    Some(fetched_book)
                }
                Err(e) => {
                    warn!(
                        domain = %domain,
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to re-fetch local domain with snapshot"
                    );
                    None
                }
            }
        } else {
            self.remote.fetch_by_correlation(domain, correlation_id).await
        }
    }
}
