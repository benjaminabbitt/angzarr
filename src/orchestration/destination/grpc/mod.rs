//! gRPC destination fetcher.
//!
//! Wraps `EventQueryClient` for fetching aggregate state from remote services.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::warn;

use crate::proto::event_query_client::EventQueryClient;
use crate::proto::{Cover, EventBook, Query};
use crate::proto_ext::correlated_request;

use super::DestinationFetcher;

/// Fetches destination state via gRPC `EventQueryClient` per domain.
#[derive(Clone)]
pub struct GrpcDestinationFetcher {
    clients: Arc<HashMap<String, Arc<Mutex<EventQueryClient<tonic::transport::Channel>>>>>,
}

impl GrpcDestinationFetcher {
    /// Create with domain -> gRPC client mapping.
    pub fn new(clients: HashMap<String, EventQueryClient<tonic::transport::Channel>>) -> Self {
        let wrapped = clients
            .into_iter()
            .map(|(k, v)| (k, Arc::new(Mutex::new(v))))
            .collect();
        Self {
            clients: Arc::new(wrapped),
        }
    }

    /// Fetch an EventBook by cover (domain + root).
    ///
    /// Exposed as a direct method for callers that need `Result` rather than `Option`.
    pub async fn fetch_result(&self, cover: &Cover) -> Result<EventBook, tonic::Status> {
        let domain = &cover.domain;
        let correlation_id = &cover.correlation_id;
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| tonic::Status::invalid_argument("Cover must have root UUID"))?;

        let client = self.clients.get(domain).ok_or_else(|| {
            tonic::Status::not_found(format!("No EventQuery registered for domain: {}", domain))
        })?;

        let query = Query {
            cover: Some(Cover {
                domain: domain.clone(),
                root: Some(root.clone()),
                correlation_id: correlation_id.clone(),
                edition: None,
            }),
            selection: None,
        };

        let mut client = client.lock().await;
        let event_book = client
            .get_event_book(correlated_request(query, correlation_id))
            .await?
            .into_inner();

        Ok(event_book)
    }
}

#[async_trait]
impl DestinationFetcher for GrpcDestinationFetcher {
    async fn fetch(&self, cover: &Cover) -> Option<EventBook> {
        match self.fetch_result(cover).await {
            Ok(book) => Some(book),
            Err(e) => {
                warn!(
                    domain = %cover.domain,
                    error = %e,
                    "Failed to fetch destination EventBook"
                );
                None
            }
        }
    }

    async fn fetch_by_correlation(&self, domain: &str, correlation_id: &str) -> Option<EventBook> {
        let client = self.clients.get(domain)?;

        let query = Query {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: None,
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            selection: None,
        };

        let mut client = client.lock().await;
        match client
            .get_event_book(correlated_request(query, correlation_id))
            .await
        {
            Ok(resp) => Some(resp.into_inner()),
            Err(e) => {
                warn!(
                    domain = %domain,
                    correlation_id = %correlation_id,
                    error = %e,
                    "Failed to fetch destination by correlation via gRPC"
                );
                None
            }
        }
    }
}
