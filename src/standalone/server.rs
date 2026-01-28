//! gRPC server bridges for standalone mode.
//!
//! Wraps in-process handlers as gRPC servers for consistency with distributed mode.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

use crate::proto::aggregate_coordinator_server::{
    AggregateCoordinator, AggregateCoordinatorServer,
};
use crate::proto::aggregate_server::{Aggregate, AggregateServer};
use crate::proto::business_response::Result as BusinessResult;
use crate::proto::event_query_server::{EventQuery as EventQueryTrait, EventQueryServer};
use crate::proto::projector_coordinator_server::{
    ProjectorCoordinator, ProjectorCoordinatorServer,
};
use crate::proto::{
    AggregateRoot, BusinessResponse, CommandBook, CommandResponse, ContextualCommand,
    DryRunRequest, EventBook, Projection, Query, SyncCommandBook, SyncEventBook, Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;

use super::router::{CommandRouter, DomainStorage};
use super::traits::{AggregateHandler, ProjectorHandler};

/// Bridge service that wraps an AggregateHandler as a gRPC Aggregate service.
pub struct AggregateHandlerBridge {
    handler: Arc<dyn AggregateHandler>,
}

impl AggregateHandlerBridge {
    /// Create a new bridge wrapping the given handler.
    pub fn new(handler: Arc<dyn AggregateHandler>) -> Self {
        Self { handler }
    }
}

#[tonic::async_trait]
impl Aggregate for AggregateHandlerBridge {
    async fn handle(
        &self,
        request: Request<ContextualCommand>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let cmd = request.into_inner();
        match self.handler.handle(cmd).await {
            Ok(events) => Ok(Response::new(BusinessResponse {
                result: Some(BusinessResult::Events(events)),
            })),
            Err(e) => Err(e),
        }
    }
}

/// Bridge service that wraps a ProjectorHandler as a gRPC ProjectorCoordinator service.
pub struct ProjectorHandlerBridge {
    handler: Arc<dyn ProjectorHandler>,
}

impl ProjectorHandlerBridge {
    /// Create a new bridge wrapping the given handler.
    pub fn new(handler: Arc<dyn ProjectorHandler>) -> Self {
        Self { handler }
    }
}

#[tonic::async_trait]
impl ProjectorCoordinator for ProjectorHandlerBridge {
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let events = request.into_inner();
        // Fire and forget - log errors but don't fail
        if let Err(e) = self.handler.handle(&events).await {
            tracing::warn!(error = %e, "Async projector handler failed");
        }
        Ok(Response::new(()))
    }

    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        let sync_book = request.into_inner();
        let events = sync_book
            .events
            .ok_or_else(|| Status::invalid_argument("Missing events"))?;

        match self.handler.handle(&events).await {
            Ok(projection) => Ok(Response::new(projection)),
            Err(e) => Err(e),
        }
    }
}

/// Info about a running gRPC server.
pub struct ServerInfo {
    /// The address the server is listening on.
    pub addr: SocketAddr,
    /// Shutdown signal sender.
    shutdown_tx: oneshot::Sender<()>,
}

impl ServerInfo {
    /// Create a ServerInfo from address and shutdown sender.
    pub fn from_parts(addr: SocketAddr, shutdown_tx: oneshot::Sender<()>) -> Self {
        Self { addr, shutdown_tx }
    }

    /// Signal the server to shut down.
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Start a gRPC server for an aggregate handler.
///
/// Returns the server info including the dynamically assigned address.
pub async fn start_aggregate_server(
    domain: &str,
    handler: Arc<dyn AggregateHandler>,
) -> Result<ServerInfo, Box<dyn std::error::Error + Send + Sync>> {
    let bridge = AggregateHandlerBridge::new(handler);
    let service = AggregateServer::new(bridge);

    // Bind to port 0 for dynamic assignment
    let addr: SocketAddr = "127.0.0.1:0".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    info!(
        domain = %domain,
        addr = %local_addr,
        "Started aggregate gRPC server"
    );

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let server = tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
                debug!("Aggregate server shutdown signal received");
            });

        if let Err(e) = server.await {
            tracing::error!(error = %e, "Aggregate server error");
        }
    });

    Ok(ServerInfo {
        addr: local_addr,
        shutdown_tx,
    })
}

/// Start a gRPC server for a projector handler.
///
/// Returns the server info including the dynamically assigned address.
pub async fn start_projector_server(
    name: &str,
    handler: Arc<dyn ProjectorHandler>,
) -> Result<ServerInfo, Box<dyn std::error::Error + Send + Sync>> {
    let bridge = ProjectorHandlerBridge::new(handler);
    let service = ProjectorCoordinatorServer::new(bridge);

    // Bind to port 0 for dynamic assignment
    let addr: SocketAddr = "127.0.0.1:0".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    info!(
        projector = %name,
        addr = %local_addr,
        "Started projector gRPC server"
    );

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let server = tonic::transport::Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
                debug!("Projector server shutdown signal received");
            });

        if let Err(e) = server.await {
            tracing::error!(error = %e, "Projector server error");
        }
    });

    Ok(ServerInfo {
        addr: local_addr,
        shutdown_tx,
    })
}

/// Bridge wrapping standalone CommandRouter as AggregateCoordinator gRPC service.
///
/// Allows the gateway to route commands through discovery to the standalone router.
pub struct CoordinatorBridge {
    router: Arc<CommandRouter>,
}

impl CoordinatorBridge {
    /// Create a new coordinator bridge wrapping the given router.
    pub fn new(router: Arc<CommandRouter>) -> Self {
        Self { router }
    }
}

#[tonic::async_trait]
impl AggregateCoordinator for CoordinatorBridge {
    async fn handle(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command = request.into_inner();
        let response = self.router.execute(command).await?;
        Ok(Response::new(response))
    }

    async fn handle_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_cmd = request.into_inner();
        let command = sync_cmd
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;
        // Standalone sync projectors are already inline — execute normally.
        let response = self.router.execute(command).await?;
        Ok(Response::new(response))
    }

    async fn dry_run_handle(
        &self,
        request: Request<DryRunRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let dry_run = request.into_inner();
        let command = dry_run
            .command
            .ok_or_else(|| Status::invalid_argument("DryRunRequest must have a command"))?;

        let (as_of_sequence, as_of_timestamp) = match dry_run.point_in_time {
            Some(temporal) => match temporal.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    (Some(seq), None)
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ref ts)) => {
                    let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(ts)
                        .map_err(|e| Status::invalid_argument(e.to_string()))?;
                    (None, Some(rfc3339))
                }
                None => (None, None),
            },
            None => (None, None),
        };

        let response = self
            .router
            .dry_run(command, as_of_sequence, as_of_timestamp.as_deref())
            .await?;
        Ok(Response::new(response))
    }
}

/// Bridge wrapping domain stores as EventQuery gRPC service.
///
/// Routes queries by domain to the appropriate event store.
pub struct StandaloneEventQueryBridge {
    stores: HashMap<String, DomainStorage>,
}

impl StandaloneEventQueryBridge {
    /// Create a new event query bridge wrapping the given domain stores.
    pub fn new(stores: HashMap<String, DomainStorage>) -> Self {
        Self { stores }
    }

    #[allow(clippy::result_large_err)]
    fn get_repo(&self, domain: &str) -> Result<EventBookRepository, Status> {
        let store = self
            .stores
            .get(domain)
            .ok_or_else(|| Status::not_found(format!("Unknown domain: {domain}")))?;
        Ok(EventBookRepository::new(
            store.event_store.clone(),
            store.snapshot_store.clone(),
        ))
    }
}

#[tonic::async_trait]
impl EventQueryTrait for StandaloneEventQueryBridge {
    async fn get_event_book(&self, request: Request<Query>) -> Result<Response<EventBook>, Status> {
        let query = request.into_inner();
        let cover = query
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Query must have a cover"))?;
        let domain = &cover.domain;
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Query must have a root UUID"))?;
        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        let repo = self.get_repo(domain)?;

        let book = match query.selection {
            Some(crate::proto::query::Selection::Range(ref range)) => {
                let lower = range.lower;
                let upper = range.upper.map(|u| u.saturating_add(1)).unwrap_or(u32::MAX);
                repo.get_from_to(domain, root_uuid, lower, upper).await
            }
            Some(crate::proto::query::Selection::Temporal(ref tq)) => match tq.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ref ts)) => {
                    let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(ts)
                        .map_err(|e| Status::invalid_argument(e.to_string()))?;
                    repo.get_temporal_by_time(domain, root_uuid, &rfc3339).await
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    repo.get_temporal_by_sequence(domain, root_uuid, seq).await
                }
                None => {
                    return Err(Status::invalid_argument(
                        "TemporalQuery must specify as_of_time or as_of_sequence",
                    ))
                }
            },
            _ => repo.get(domain, root_uuid).await,
        }
        .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(book))
    }

    type GetEventsStream = ReceiverStream<Result<EventBook, Status>>;

    async fn get_events(
        &self,
        request: Request<Query>,
    ) -> Result<Response<Self::GetEventsStream>, Status> {
        let query = request.into_inner();
        let cover = query
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Query must have a cover"))?;
        let domain = cover.domain.clone();
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Query must have a root UUID"))?;
        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        let repo = self.get_repo(&domain)?;
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            match repo.get(&domain, root_uuid).await {
                Ok(book) => {
                    let _ = tx.send(Ok(book)).await;
                }
                Err(e) => {
                    let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type SynchronizeStream = ReceiverStream<Result<EventBook, Status>>;

    async fn synchronize(
        &self,
        _request: Request<tonic::Streaming<Query>>,
    ) -> Result<Response<Self::SynchronizeStream>, Status> {
        Err(Status::unimplemented(
            "Synchronize not available in standalone mode",
        ))
    }

    type GetAggregateRootsStream = ReceiverStream<Result<AggregateRoot, Status>>;

    async fn get_aggregate_roots(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetAggregateRootsStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let stores = self.stores.clone();

        tokio::spawn(async move {
            for (domain, storage) in &stores {
                match storage.event_store.list_roots(domain).await {
                    Ok(roots) => {
                        for root in roots {
                            let aggregate = AggregateRoot {
                                domain: domain.clone(),
                                root: Some(ProtoUuid {
                                    value: root.as_bytes().to_vec(),
                                }),
                            };
                            if tx.send(Ok(aggregate)).await.is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!(domain = %domain, error = %e, "Failed to list roots");
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Start a bridge gRPC server hosting both AggregateCoordinator and EventQuery.
///
/// Used by the standalone gateway to make the in-process router and stores
/// accessible via service discovery.
pub async fn start_bridge_server(
    router: Arc<CommandRouter>,
    stores: HashMap<String, DomainStorage>,
) -> Result<ServerInfo, Box<dyn std::error::Error + Send + Sync>> {
    let coordinator = CoordinatorBridge::new(router);
    let event_query = StandaloneEventQueryBridge::new(stores);

    let addr: SocketAddr = "127.0.0.1:0".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    info!(addr = %local_addr, "Started gateway bridge gRPC server");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(async move {
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
        let server = tonic::transport::Server::builder()
            .add_service(AggregateCoordinatorServer::new(coordinator))
            .add_service(EventQueryServer::new(event_query))
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_rx.await;
                debug!("Bridge server shutdown signal received");
            });

        if let Err(e) = server.await {
            error!(error = %e, "Bridge server error");
        }
    });

    Ok(ServerInfo {
        addr: local_addr,
        shutdown_tx,
    })
}
