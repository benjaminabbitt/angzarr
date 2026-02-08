//! Local (in-process) destination fetcher.
//!
//! Reads aggregate state directly from in-process `DomainStorage` maps.
//! Uses `EventBookRepository` to properly load snapshots and only subsequent events.

use std::collections::HashMap;

use async_trait::async_trait;
use tracing::error;

use crate::proto::{Cover, EventBook};
use crate::proto_ext::CoverExt;
use crate::repository::EventBookRepository;
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
        let edition = cover.edition();
        let root_uuid = cover
            .root
            .as_ref()
            .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())?;

        let store = self.domain_stores.get(domain)?;

        // Use EventBookRepository to properly load snapshot + subsequent events
        let repo = EventBookRepository::new(
            store.event_store.clone(),
            store.snapshot_store.clone(),
        );

        match repo.get(domain, edition, root_uuid).await {
            Ok(mut book) => {
                // Preserve the correlation_id from the input cover
                if let Some(ref mut book_cover) = book.cover {
                    book_cover.correlation_id = cover.correlation_id.clone();
                }
                Some(book)
            }
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

        // First find the aggregate by correlation_id
        let books = match store.event_store.get_by_correlation(correlation_id).await {
            Ok(books) => books,
            Err(e) => {
                error!(
                    domain = %domain,
                    correlation_id = %correlation_id,
                    error = %e,
                    "Failed to fetch destination by correlation"
                );
                return None;
            }
        };

        // Find the first book matching this domain
        let book = books.into_iter().find(|b| {
            b.cover.as_ref().is_some_and(|c| c.domain == domain)
        })?;

        // Re-fetch using EventBookRepository to get snapshot-optimized version
        let cover = book.cover.as_ref()?;
        let edition = cover.edition.as_ref().map(|e| e.name.as_str()).unwrap_or("main");
        let root_uuid = cover.root.as_ref()
            .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())?;

        let repo = EventBookRepository::new(
            store.event_store.clone(),
            store.snapshot_store.clone(),
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
                error!(
                    domain = %domain,
                    correlation_id = %correlation_id,
                    error = %e,
                    "Failed to re-fetch destination with snapshot"
                );
                None
            }
        }
    }
}
