//! Hybrid destination fetcher for process manager sidecars.
//!
//! Routes PM domain queries to local event store, all others to gRPC.
//! Solves the chicken-and-egg problem: PM sidecar needs to query its own
//! domain's state, but there's no aggregate service for the PM domain.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::proto::{Cover, EventBook};
use crate::proto_ext::CoverExt;
use crate::storage::EventStore;

use super::DestinationFetcher;

/// Fetches from local event store for one domain, delegates others to a remote fetcher.
///
/// Used by the process manager sidecar to access its own state directly while
/// routing other domain queries to remote aggregate sidecars via gRPC.
pub struct HybridDestinationFetcher {
    local_domain: String,
    local_store: Arc<dyn EventStore>,
    remote: Arc<dyn DestinationFetcher>,
}

impl HybridDestinationFetcher {
    /// Create with the local domain name, event store, and remote fetcher for other domains.
    pub fn new(
        local_domain: String,
        local_store: Arc<dyn EventStore>,
        remote: Arc<dyn DestinationFetcher>,
    ) -> Self {
        Self {
            local_domain,
            local_store,
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

            match self.local_store.get(&self.local_domain, edition, root).await {
                Ok(pages) => Some(EventBook {
                    cover: Some(cover.clone()),
                    pages,
                    snapshot: None,
                    snapshot_state: None,
                }),
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
            match self.local_store.get_by_correlation(correlation_id).await {
                Ok(books) => books
                    .into_iter()
                    .find(|b| b.cover.as_ref().is_some_and(|c| c.domain == self.local_domain)),
                Err(e) => {
                    warn!(
                        domain = %domain,
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to fetch local domain state by correlation"
                    );
                    None
                }
            }
        } else {
            self.remote.fetch_by_correlation(domain, correlation_id).await
        }
    }
}
