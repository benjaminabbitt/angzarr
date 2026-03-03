//! gRPC services for standalone mode.
//!
//! Per-domain implementations of `AggregateCoordinator` and `EventQuery` that wrap the
//! standalone `CommandRouter` and domain stores. No intermediate bridge servers
//! or service discovery needed.
//!
//! Edition handling is implicit: the router extracts edition from the command's
//! Cover and passes it to the event store, which handles composite reads.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::error;

use crate::config::ResourceLimits;
use crate::proto::command_handler_coordinator_service_server::CommandHandlerCoordinatorService;
use crate::proto::event_query_service_server::EventQueryService as EventQueryTrait;
use crate::proto::{
    AggregateRoot, BusinessResponse, CommandBook, CommandRequest, CommandResponse, EventBook,
    EventRequest, FactInjectionResponse, Query, SpeculateCommandHandlerRequest, Uuid as ProtoUuid,
};
use crate::proto_ext::CoverExt;
use crate::repository::EventBookRepository;
use crate::validation;

use crate::orchestration::aggregate::DEFAULT_EDITION;

use super::router::{CommandRouter, DomainStorage};

/// Per-domain `AggregateCoordinator` implementation for standalone mode.
///
/// Routes commands to the standalone `CommandRouter` after validating they
/// belong to this service's domain. Matches the distributed mode pattern
/// where each domain has its own coordinator endpoint.
pub struct StandaloneAggregateService {
    domain: String,
    router: Arc<CommandRouter>,
    limits: ResourceLimits,
}

impl StandaloneAggregateService {
    /// Create a new per-domain aggregate service.
    pub fn new(domain: impl Into<String>, router: Arc<CommandRouter>) -> Self {
        Self::with_limits(domain, router, ResourceLimits::default())
    }

    /// Create a new per-domain aggregate service with custom resource limits.
    pub fn with_limits(
        domain: impl Into<String>,
        router: Arc<CommandRouter>,
        limits: ResourceLimits,
    ) -> Self {
        Self {
            domain: domain.into(),
            router,
            limits,
        }
    }

    /// Validate that the command is for this service's domain.
    #[allow(clippy::result_large_err)]
    fn validate_domain(&self, command: &CommandBook) -> Result<(), Status> {
        validate_domain_match(command.domain(), &self.domain, "Command")
    }
}

#[tonic::async_trait]
impl CommandHandlerCoordinatorService for StandaloneAggregateService {
    async fn handle_command(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let sync_cmd = request.into_inner();
        let command = sync_cmd.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::COMMAND_REQUEST_MISSING_COMMAND)
        })?;
        self.validate_domain(&command)?;
        validation::validate_command_book(&command, &self.limits)?;
        // Standalone mode doesn't differentiate sync modes - all execution is synchronous
        let response = self.router.execute(command).await?;
        Ok(Response::new(response))
    }

    async fn handle_sync_speculative(
        &self,
        request: Request<SpeculateCommandHandlerRequest>,
    ) -> Result<Response<CommandResponse>, Status> {
        let speculate_req = request.into_inner();
        let command = speculate_req.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::SPECULATE_CMD_MISSING_COMMAND)
        })?;
        self.validate_domain(&command)?;

        let (as_of_sequence, as_of_timestamp) =
            parse_temporal_query(speculate_req.point_in_time.as_ref());

        let response = self
            .router
            .speculative(command, as_of_sequence, as_of_timestamp.as_deref())
            .await?;
        Ok(Response::new(response))
    }

    async fn handle_compensation(
        &self,
        request: Request<CommandRequest>,
    ) -> Result<Response<BusinessResponse>, Status> {
        let sync_cmd = request.into_inner();
        let command = sync_cmd.command.ok_or_else(|| {
            Status::invalid_argument(super::errmsg::COMMAND_REQUEST_MISSING_COMMAND)
        })?;
        self.validate_domain(&command)?;
        validation::validate_command_book(&command, &self.limits)?;
        let response = self.router.execute_compensation(command).await?;
        Ok(Response::new(response))
    }

    async fn handle_event(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<FactInjectionResponse>, Status> {
        let sync_event_book = request.into_inner();
        let fact_events = sync_event_book
            .events
            .ok_or_else(|| Status::invalid_argument(super::errmsg::EVENT_REQUEST_MISSING_EVENTS))?;

        // Validate domain matches
        let fact_domain = fact_events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");
        if fact_domain != self.domain {
            return Err(Status::invalid_argument(format!(
                "Event domain '{}' does not match service domain '{}'",
                fact_domain, self.domain
            )));
        }

        let fact_response = self
            .router
            .inject_fact(fact_events, sync_event_book.route_to_handler)
            .await?;

        Ok(Response::new(FactInjectionResponse {
            events: Some(fact_response.events),
            already_processed: fact_response.already_processed,
            projections: fact_response.projections,
        }))
    }
}

/// Per-domain `EventQuery` implementation for standalone mode.
///
/// Routes queries directly to the domain's event store. Validates that queries
/// are for this service's domain.
pub struct SingleDomainEventQuery {
    domain: String,
    storage: DomainStorage,
}

impl SingleDomainEventQuery {
    /// Create a new per-domain event query service.
    pub fn new(domain: impl Into<String>, storage: DomainStorage) -> Self {
        Self {
            domain: domain.into(),
            storage,
        }
    }

    /// Validate that the query is for this service's domain.
    #[allow(clippy::result_large_err)]
    fn validate_domain(&self, query: &Query) -> Result<(), Status> {
        let query_domain = query
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");
        validate_domain_match(query_domain, &self.domain, "Query")
    }

    fn get_repo(&self) -> EventBookRepository {
        EventBookRepository::with_config(
            self.storage.event_store.clone(),
            self.storage.snapshot_store.clone(),
            false,
        )
    }
}

#[tonic::async_trait]
impl EventQueryTrait for SingleDomainEventQuery {
    async fn get_event_book(&self, request: Request<Query>) -> Result<Response<EventBook>, Status> {
        let query = request.into_inner();
        self.validate_domain(&query)?;

        let cover = query
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::QUERY_MISSING_COVER))?;
        let edition = cover.edition_opt();
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::QUERY_MISSING_ROOT))?;
        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        let repo = self.get_repo();

        let book = match query.selection {
            Some(crate::proto::query::Selection::Range(ref range)) => {
                let lower = range.lower;
                let upper = range.upper.map(|u| u.saturating_add(1)).unwrap_or(u32::MAX);
                repo.get_from_to(
                    &self.domain,
                    edition.unwrap_or(DEFAULT_EDITION),
                    root_uuid,
                    lower,
                    upper,
                )
                .await
            }
            Some(crate::proto::query::Selection::Temporal(ref tq)) => match tq.point_in_time {
                Some(crate::proto::temporal_query::PointInTime::AsOfTime(ref ts)) => {
                    let rfc3339 = crate::storage::helpers::timestamp_to_rfc3339(ts)
                        .map_err(|e| Status::invalid_argument(e.to_string()))?;
                    repo.get_temporal_by_time(
                        &self.domain,
                        edition.unwrap_or(DEFAULT_EDITION),
                        root_uuid,
                        &rfc3339,
                    )
                    .await
                }
                Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => {
                    repo.get_temporal_by_sequence(
                        &self.domain,
                        edition.unwrap_or(DEFAULT_EDITION),
                        root_uuid,
                        seq,
                    )
                    .await
                }
                None => {
                    return Err(Status::invalid_argument(
                        "TemporalQuery must specify as_of_time or as_of_sequence",
                    ))
                }
            },
            _ => {
                repo.get(&self.domain, edition.unwrap_or(DEFAULT_EDITION), root_uuid)
                    .await
            }
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
        self.validate_domain(&query)?;

        let cover = query
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::QUERY_MISSING_COVER))?;
        let edition = cover.edition_opt().map(String::from);
        let root = cover
            .root
            .as_ref()
            .ok_or_else(|| Status::invalid_argument(super::errmsg::QUERY_MISSING_ROOT))?;
        let root_uuid = uuid::Uuid::from_slice(&root.value)
            .map_err(|e| Status::invalid_argument(format!("Invalid UUID: {e}")))?;

        let repo = self.get_repo();
        let domain = self.domain.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            match repo
                .get(
                    &domain,
                    edition.as_deref().unwrap_or(DEFAULT_EDITION),
                    root_uuid,
                )
                .await
            {
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
        let domain = self.domain.clone();
        let storage = self.storage.clone();

        tokio::spawn(async move {
            match storage
                .event_store
                .list_roots(&domain, DEFAULT_EDITION)
                .await
            {
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
        });

        Ok(Response::new(ReceiverStream::new(rx)))
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

// ============================================================================
// Pure Helper Functions (testable without infrastructure)
// ============================================================================

/// Validate that actual domain matches expected domain.
///
/// Returns an error with context about which component and mismatched values.
#[allow(clippy::result_large_err)]
fn validate_domain_match(actual: &str, expected: &str, context: &str) -> Result<(), Status> {
    if actual != expected {
        return Err(Status::invalid_argument(format!(
            "{} domain '{}' does not match service domain '{}'",
            context, actual, expected
        )));
    }
    Ok(())
}

/// Parse temporal query into (as_of_sequence, as_of_timestamp) tuple.
///
/// Returns (None, None) if no temporal query is specified.
fn parse_temporal_query(
    point_in_time: Option<&crate::proto::TemporalQuery>,
) -> (Option<u32>, Option<String>) {
    match point_in_time {
        Some(temporal) => match temporal.point_in_time {
            Some(crate::proto::temporal_query::PointInTime::AsOfSequence(seq)) => (Some(seq), None),
            Some(crate::proto::temporal_query::PointInTime::AsOfTime(ref ts)) => {
                let ts_str = format!("{}.{}", ts.seconds, ts.nanos);
                (None, Some(ts_str))
            }
            None => (None, None),
        },
        None => (None, None),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // validate_domain_match Tests
    // ========================================================================

    #[test]
    fn test_validate_domain_match_same_domain_succeeds() {
        let result = validate_domain_match("orders", "orders", "Command");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_domain_match_different_domain_fails() {
        let result = validate_domain_match("inventory", "orders", "Command");
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert!(status.message().contains("inventory"));
        assert!(status.message().contains("orders"));
    }

    #[test]
    fn test_validate_domain_match_empty_domain_fails() {
        let result = validate_domain_match("", "orders", "Query");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_domain_match_context_in_message() {
        let result = validate_domain_match("a", "b", "Event");
        let status = result.unwrap_err();
        assert!(status.message().contains("Event"));
    }

    // ========================================================================
    // parse_temporal_query Tests
    // ========================================================================

    #[test]
    fn test_parse_temporal_query_none_returns_none_none() {
        let (seq, ts) = parse_temporal_query(None);
        assert!(seq.is_none());
        assert!(ts.is_none());
    }

    #[test]
    fn test_parse_temporal_query_as_of_sequence() {
        use crate::proto::{temporal_query::PointInTime, TemporalQuery};

        let temporal = TemporalQuery {
            point_in_time: Some(PointInTime::AsOfSequence(42)),
        };
        let (seq, ts) = parse_temporal_query(Some(&temporal));
        assert_eq!(seq, Some(42));
        assert!(ts.is_none());
    }

    #[test]
    fn test_parse_temporal_query_as_of_time() {
        use crate::proto::{temporal_query::PointInTime, TemporalQuery};

        let temporal = TemporalQuery {
            point_in_time: Some(PointInTime::AsOfTime(prost_types::Timestamp {
                seconds: 1704067200,
                nanos: 123456789,
            })),
        };
        let (seq, ts) = parse_temporal_query(Some(&temporal));
        assert!(seq.is_none());
        assert_eq!(ts, Some("1704067200.123456789".to_string()));
    }

    #[test]
    fn test_parse_temporal_query_empty_point_in_time() {
        use crate::proto::TemporalQuery;

        let temporal = TemporalQuery {
            point_in_time: None,
        };
        let (seq, ts) = parse_temporal_query(Some(&temporal));
        assert!(seq.is_none());
        assert!(ts.is_none());
    }
}
