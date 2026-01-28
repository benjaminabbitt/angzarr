//! Destination fetching abstraction.
//!
//! `DestinationFetcher` loads EventBook state for saga/PM destinations.
//! - `local/`: reads from in-process `DomainStorage` maps
//! - `grpc/`: wraps `EventQueryClient` for remote fetching

pub mod grpc;
#[cfg(feature = "sqlite")]
pub mod local;

use async_trait::async_trait;

use crate::proto::{Cover, EventBook};

/// Fetches aggregate state for saga/PM destination resolution.
#[async_trait]
pub trait DestinationFetcher: Send + Sync {
    /// Fetch state by cover (domain + root or correlation_id).
    async fn fetch(&self, cover: &Cover) -> Option<EventBook>;

    /// Fetch state by correlation ID within a specific domain.
    async fn fetch_by_correlation(
        &self,
        domain: &str,
        correlation_id: &str,
    ) -> Option<EventBook>;
}
