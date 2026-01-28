//! Local (in-process) destination fetcher.
//!
//! Reads aggregate state directly from in-process `DomainStorage` maps.

use std::collections::HashMap;

use async_trait::async_trait;
use tracing::error;

use crate::proto::{Cover, EventBook, Uuid as ProtoUuid};
use crate::standalone::DomainStorage;

use super::DestinationFetcher;

/// Fetches destination state from in-process event stores.
pub struct LocalDestinationFetcher {
    domain_stores: HashMap<String, DomainStorage>,
}

impl LocalDestinationFetcher {
    /// Create with per-domain storage map.
    pub fn new(domain_stores: HashMap<String, DomainStorage>) -> Self {
        Self { domain_stores }
    }
}

#[async_trait]
impl DestinationFetcher for LocalDestinationFetcher {
    async fn fetch(&self, cover: &Cover) -> Option<EventBook> {
        let domain = &cover.domain;
        let root_uuid = cover
            .root
            .as_ref()
            .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())?;

        let store = self.domain_stores.get(domain)?;

        match store.event_store.get(domain, root_uuid).await {
            Ok(pages) => Some(EventBook {
                cover: Some(Cover {
                    domain: domain.clone(),
                    root: Some(ProtoUuid {
                        value: root_uuid.as_bytes().to_vec(),
                    }),
                    correlation_id: cover.correlation_id.clone(),
                }),
                pages,
                snapshot: None,
                snapshot_state: None,
            }),
            Err(e) => {
                error!(
                    domain = %domain,
                    error = %e,
                    "Failed to fetch destination EventBook"
                );
                None
            }
        }
    }

    async fn fetch_by_correlation(&self, domain: &str, correlation_id: &str) -> Option<EventBook> {
        let store = self.domain_stores.get(domain)?;

        match store.event_store.get_by_correlation(correlation_id).await {
            Ok(books) => books.into_iter().next(),
            Err(e) => {
                error!(
                    domain = %domain,
                    correlation_id = %correlation_id,
                    error = %e,
                    "Failed to fetch destination by correlation"
                );
                None
            }
        }
    }
}
