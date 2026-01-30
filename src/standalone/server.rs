//! gRPC services for standalone mode.
//!
//! Direct implementations of `CommandGateway` and `EventQuery` that wrap the
//! standalone `CommandRouter` and domain stores. No intermediate bridge servers
//! or service discovery needed.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::error;

use crate::proto::command_gateway_server::CommandGateway;
use crate::proto::event_query_server::EventQuery as EventQueryTrait;
use crate::proto::{
    AggregateRoot, CommandBook, CommandResponse, DryRunRequest, EventBook, Query, SyncCommandBook,
    Uuid as ProtoUuid,
};
use crate::repository::EventBookRepository;

use crate::orchestration::aggregate::DEFAULT_EDITION;

use super::edition::EditionManager;
use super::router::{CommandRouter, DomainStorage};

/// Standalone `CommandGateway` implementation.
///
/// Routes commands directly to the standalone `CommandRouter` without
/// service discovery or intermediate gRPC bridges. Commands with a
/// `Cover.edition` field are routed to the edition manager.
pub struct StandaloneGatewayService {
    router: Arc<CommandRouter>,
    edition_manager: Arc<EditionManager>,
}

impl StandaloneGatewayService {
    /// Create a new standalone gateway wrapping the given router and edition manager.
    pub fn new(router: Arc<CommandRouter>, edition_manager: Arc<EditionManager>) -> Self {
        Self {
            router,
            edition_manager,
        }
    }
}

#[tonic::async_trait]
impl CommandGateway for StandaloneGatewayService {
    async fn execute(
        &self,
        request: Request<CommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let command = request.into_inner();
        let edition_name = command
            .cover
            .as_ref()
            .and_then(|c| c.edition.clone())
            .filter(|e| !e.is_empty());
        let response = match edition_name.as_deref() {
            None | Some(DEFAULT_EDITION) => self.router.execute(command).await?,
            Some(name) => self.edition_manager.execute(name, command).await?,
        };
        Ok(Response::new(response))
    }

    async fn execute_sync(
        &self,
        request: Request<SyncCommandBook>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_cmd = request.into_inner();
        let command = sync_cmd
            .command
            .ok_or_else(|| Status::invalid_argument("SyncCommandBook must have a command"))?;
        // Standalone sync projectors are inline — execute normally.
        let response = self.router.execute(command).await?;
        Ok(Response::new(response))
    }

    type ExecuteStreamStream =
        Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    async fn execute_stream(
        &self,
        _request: Request<CommandBook>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        Err(Status::unimplemented(
            "Event streaming not available in standalone mode",
        ))
    }

    async fn dry_run_execute(
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

/// Standalone `EventQuery` implementation.
///
/// Routes queries by domain to the appropriate event store directly,
/// without service discovery.
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
        // Disable snapshot reading — event queries return full event history,
        // not snapshot-optimized views for aggregate state reconstruction.
        Ok(EventBookRepository::with_config(
            store.event_store.clone(),
            store.snapshot_store.clone(),
            false,
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
                repo.get_from_to(domain, DEFAULT_EDITION, root_uuid, lower, upper).await
            }
            Some(crate::proto::query::Selection::Temporal(ref tq)) => match tq.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ref ts)) => {
                    let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(ts)
                        .map_err(|e| Status::invalid_argument(e.to_string()))?;
                    repo.get_temporal_by_time(domain, DEFAULT_EDITION, root_uuid, &rfc3339).await
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    repo.get_temporal_by_sequence(domain, DEFAULT_EDITION, root_uuid, seq).await
                }
                None => {
                    return Err(Status::invalid_argument(
                        "TemporalQuery must specify as_of_time or as_of_sequence",
                    ))
                }
            },
            _ => repo.get(domain, DEFAULT_EDITION, root_uuid).await,
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
            match repo.get(&domain, DEFAULT_EDITION, root_uuid).await {
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
                match storage.event_store.list_roots(domain, DEFAULT_EDITION).await {
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
