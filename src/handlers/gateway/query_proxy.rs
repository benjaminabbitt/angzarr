//! Event query proxy that routes queries to aggregate sidecars based on domain.
//!
//! Provides domain-based routing for EventQuery operations, forwarding requests
//! to the appropriate aggregate sidecar based on service discovery.

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

use crate::discovery::ServiceDiscovery;
use crate::proto::event_query_server::EventQuery;
use crate::proto::{AggregateRoot, EventBook, Query};
use crate::proto_ext::{correlated_request, CoverExt};

use super::map_discovery_error;

/// Event query proxy that routes queries to aggregate sidecars based on domain.
pub struct EventQueryProxy {
    discovery: Arc<dyn ServiceDiscovery>,
}

impl EventQueryProxy {
    /// Create a new event query proxy with service discovery for domain routing.
    pub fn new(discovery: Arc<dyn ServiceDiscovery>) -> Self {
        Self { discovery }
    }
}

#[tonic::async_trait]
impl EventQuery for EventQueryProxy {
    /// Get a single EventBook for the domain/root.
    async fn get_event_book(&self, request: Request<Query>) -> Result<Response<EventBook>, Status> {
        let query = request.into_inner();
        let domain = query.domain();
        let correlation_id = query.correlation_id().to_string();

        debug!(domain = %domain, "Proxying GetEventBook query");

        let mut client = self
            .discovery
            .get_event_query(domain)
            .await
            .map_err(map_discovery_error)?;
        client
            .get_event_book(correlated_request(query, &correlation_id))
            .await
    }

    type GetEventsStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Stream EventBooks for the domain/root.
    async fn get_events(
        &self,
        request: Request<Query>,
    ) -> Result<Response<Self::GetEventsStream>, Status> {
        let query = request.into_inner();
        let domain = query.domain().to_string();
        let correlation_id = query.correlation_id().to_string();

        debug!(domain = %domain, "Proxying GetEvents query");

        let mut client = self
            .discovery
            .get_event_query(&domain)
            .await
            .map_err(map_discovery_error)?;
        let response = client
            .get_events(correlated_request(query, &correlation_id))
            .await?;

        // Re-box the stream to match our return type
        let stream = response.into_inner();
        Ok(Response::new(Box::pin(stream)))
    }

    type SynchronizeStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    /// Bidirectional synchronization stream.
    ///
    /// Forwards queries to the appropriate aggregate sidecar and streams back events.
    /// Properly handles client disconnect by stopping the forwarding task.
    async fn synchronize(
        &self,
        request: Request<tonic::Streaming<Query>>,
    ) -> Result<Response<Self::SynchronizeStream>, Status> {
        // For synchronize, we need to handle multiple domains potentially
        // For now, route based on the first query's domain
        let mut inbound = request.into_inner();

        // Peek at first message to determine domain
        let first_query = match inbound.next().await {
            Some(Ok(q)) => q,
            Some(Err(e)) => return Err(e),
            None => return Err(Status::invalid_argument("No queries provided")),
        };

        let domain = first_query.domain().to_string();
        let correlation_id = first_query.correlation_id().to_string();
        debug!(domain = %domain, "Proxying Synchronize stream");

        let mut client = self
            .discovery
            .get_event_query(&domain)
            .await
            .map_err(map_discovery_error)?;

        // Create a channel to forward queries including the first one
        let (query_tx, query_rx) = mpsc::channel(32);
        let _ = query_tx.send(first_query).await;

        // Forward remaining queries, stopping if downstream closes
        let domain_clone = domain.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Downstream closed - stop forwarding
                    _ = query_tx.closed() => {
                        debug!(domain = %domain_clone, "Synchronize downstream closed, stopping query forwarding");
                        break;
                    }
                    // Receive next query from client
                    result = inbound.next() => {
                        match result {
                            Some(Ok(query)) => {
                                if query_tx.send(query).await.is_err() {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!(domain = %domain_clone, error = %e, "Synchronize inbound stream error");
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let outbound = ReceiverStream::new(query_rx);
        let response = client
            .synchronize(correlated_request(outbound, &correlation_id))
            .await?;

        Ok(Response::new(Box::pin(response.into_inner())))
    }

    type GetAggregateRootsStream =
        Pin<Box<dyn Stream<Item = Result<AggregateRoot, Status>> + Send + 'static>>;

    /// Get all aggregate roots across all domains.
    ///
    /// Queries all registered aggregate sidecars and merges results.
    /// Properly handles client disconnect by stopping mid-query.
    async fn get_aggregate_roots(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetAggregateRootsStream>, Status> {
        // This needs to query all registered domains and merge results
        let domains = self.discovery.aggregate_domains().await;

        let (tx, rx) = mpsc::channel(32);
        let discovery = self.discovery.clone();

        tokio::spawn(async move {
            'domains: for domain in domains {
                // Check if client disconnected before starting next domain
                if tx.is_closed() {
                    info!("GetAggregateRoots client disconnected, stopping");
                    break;
                }

                match discovery.get_event_query(&domain).await {
                    Ok(mut client) => {
                        if let Ok(response) = client.get_aggregate_roots(Request::new(())).await {
                            let mut stream = response.into_inner();
                            loop {
                                tokio::select! {
                                    // Client disconnected - stop immediately
                                    _ = tx.closed() => {
                                        info!(domain = %domain, "GetAggregateRoots client disconnected during stream");
                                        break 'domains;
                                    }
                                    // Next result from domain
                                    result = stream.next() => {
                                        match result {
                                            Some(Ok(root)) => {
                                                if tx.send(Ok(root)).await.is_err() {
                                                    break 'domains;
                                                }
                                            }
                                            Some(Err(e)) => {
                                                warn!(domain = %domain, error = %e, "Error streaming aggregate roots");
                                                break; // Continue to next domain
                                            }
                                            None => break, // Domain stream ended, continue to next
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(domain = %domain, error = %e, "Failed to get client for domain");
                    }
                }
            }
            debug!("GetAggregateRoots task ending");
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}
