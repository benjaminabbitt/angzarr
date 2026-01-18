//! Saga coordinator service.
//!
//! Receives events from the event bus and distributes them to registered sagas.
//! Sagas can produce new commands in response to events, enabling cross-aggregate
//! workflows. Ensures sagas receive complete EventBooks by fetching missing history
//! from the EventQuery service when needed.

use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::transport::Channel;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

use crate::clients::ServiceEndpoint;
use crate::grpc::connect_channel;
use crate::proto::{
    event_page::Sequence, saga_client::SagaClient, saga_coordinator_server::SagaCoordinator,
    EventBook, SagaCommandOrigin, SagaResponse, SyncEventBook,
};
use crate::services::event_book_repair::EventBookRepairer;
use crate::services::repairable::RepairableCoordinator;

/// Connected saga client.
struct SagaConnection {
    config: ServiceEndpoint,
    client: SagaClient<Channel>,
}

/// Saga coordinator service.
///
/// Distributes events to all registered sagas and collects responses.
/// Before forwarding, checks if EventBooks are complete and fetches
/// missing history from the EventQuery service if needed.
pub struct SagaCoordinatorService {
    sagas: Arc<RwLock<Vec<SagaConnection>>>,
    repairer: RepairableCoordinator,
}

impl SagaCoordinatorService {
    /// Create a new saga coordinator without repair capability.
    pub fn new() -> Self {
        Self {
            sagas: Arc::new(RwLock::new(Vec::new())),
            repairer: RepairableCoordinator::new(),
        }
    }

    /// Create a new saga coordinator with repair capability.
    ///
    /// The repairer will fetch missing events from the EventQuery service
    /// at the given address when incomplete EventBooks are received.
    pub async fn with_repair(event_query_address: &str) -> Result<Self, String> {
        Ok(Self {
            sagas: Arc::new(RwLock::new(Vec::new())),
            repairer: RepairableCoordinator::with_repair(event_query_address).await?,
        })
    }

    /// Create a new saga coordinator with an existing repairer.
    pub fn with_repairer(repairer: EventBookRepairer) -> Self {
        Self {
            sagas: Arc::new(RwLock::new(Vec::new())),
            repairer: RepairableCoordinator::with_repairer(repairer),
        }
    }

    /// Register a saga endpoint.
    pub async fn add_saga(&self, config: ServiceEndpoint) -> Result<(), String> {
        let channel = connect_channel(&config.address).await?;
        let client = SagaClient::new(channel);

        info!(
            saga = %config.name,
            address = %config.address,
            "Registered saga"
        );

        self.sagas
            .write()
            .await
            .push(SagaConnection { config, client });

        Ok(())
    }
}

impl Default for SagaCoordinatorService {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the triggering event sequence from an EventBook.
///
/// Returns the highest sequence number from the EventBook's pages,
/// or 0 if no events are present.
fn extract_triggering_sequence(book: &EventBook) -> u32 {
    book.pages
        .iter()
        .filter_map(|page| match &page.sequence {
            Some(Sequence::Num(n)) => Some(*n),
            Some(Sequence::Force(_)) => None,
            None => None,
        })
        .max()
        .unwrap_or(0)
}

/// Build a SagaCommandOrigin from an EventBook and saga name.
fn build_saga_origin(book: &EventBook, saga_name: &str) -> Option<SagaCommandOrigin> {
    let cover = book.cover.as_ref()?;

    Some(SagaCommandOrigin {
        saga_name: saga_name.to_string(),
        triggering_aggregate: Some(cover.clone()),
        triggering_event_sequence: extract_triggering_sequence(book),
    })
}

#[tonic::async_trait]
impl SagaCoordinator for SagaCoordinatorService {
    /// Handle events asynchronously (fire and forget).
    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let event_book = request.into_inner();

        // Repair EventBook if incomplete
        let event_book = self.repairer.repair_event_book(event_book).await?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let sagas = self.sagas.read().await;
            sagas
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        for (config, mut client) in connections {
            let req = Request::new(event_book.clone());
            match client.handle(req).await {
                Ok(_) => info!(saga.name = %config.name, "Async saga queued"),
                Err(e) => warn!(saga.name = %config.name, error = %e, "Failed to queue saga"),
            }
        }

        Ok(Response::new(()))
    }

    /// Handle events synchronously, collecting all resulting commands.
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<SagaResponse>, Status> {
        let sync_request = request.into_inner();
        let event_book = sync_request
            .events
            .ok_or_else(|| Status::invalid_argument("SyncEventBook must have events"))?;
        let correlation_id = event_book.correlation_id.clone();

        // Repair EventBook if incomplete
        let event_book = self.repairer.repair_event_book(event_book).await?;

        // Clone connections to minimize lock scope during async I/O
        let connections: Vec<_> = {
            let sagas = self.sagas.read().await;
            sagas
                .iter()
                .map(|conn| (conn.config.clone(), conn.client.clone()))
                .collect()
        };

        let mut all_commands = Vec::new();

        for (config, mut client) in connections {
            let req = Request::new(event_book.clone());
            match client.handle(req).await {
                Ok(response) => {
                    info!(saga.name = %config.name, "Synchronous saga completed");
                    let inner = response.into_inner();

                    // Build saga origin for tracking compensation flow
                    let saga_origin = build_saga_origin(&event_book, &config.name);

                    // Propagate correlation_id and saga_origin to saga-produced commands
                    for mut cmd in inner.commands {
                        if cmd.correlation_id.is_empty() {
                            cmd.correlation_id = correlation_id.clone();
                        }
                        // Set saga_origin if not already set by the saga
                        if cmd.saga_origin.is_none() {
                            cmd.saga_origin = saga_origin.clone();
                        }
                        all_commands.push(cmd);
                    }
                }
                Err(e) => {
                    error!(saga.name = %config.name, error = %e, "Synchronous saga failed");
                    return Err(Status::internal(format!(
                        "Saga {} failed: {}",
                        config.name, e
                    )));
                }
            }
        }

        Ok(Response::new(SagaResponse {
            commands: all_commands,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::ServiceEndpoint;
    use crate::proto::{event_page, Cover, EventPage, Uuid as ProtoUuid};
    use prost_types::Any;

    fn make_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    fn make_complete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(0)),
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    fn make_incomplete_event_book() -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![EventPage {
                sequence: Some(event_page::Sequence::Num(5)), // Missing events 0-4
                event: Some(Any {
                    type_url: "test.Event".to_string(),
                    value: vec![],
                }),
                created_at: None,
            }],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_new_creates_empty_coordinator() {
        let coordinator = SagaCoordinatorService::new();
        assert!(coordinator.sagas.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_default_creates_empty_coordinator() {
        let coordinator = SagaCoordinatorService::default();
        assert!(coordinator.sagas.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_handle_sync_with_no_sagas_returns_empty_response() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_event_book();
        let sync_request = SyncEventBook {
            events: Some(event_book),
            sync_mode: crate::proto::SyncMode::Simple.into(),
        };

        let response = coordinator.handle_sync(Request::new(sync_request)).await;

        assert!(response.is_ok());
        let sync_response = response.unwrap().into_inner();
        assert!(sync_response.commands.is_empty());
    }

    #[tokio::test]
    async fn test_handle_with_no_sagas_succeeds() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_event_book();

        let response = coordinator.handle(Request::new(event_book)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_add_saga_invalid_address() {
        let coordinator = SagaCoordinatorService::new();
        let config = ServiceEndpoint {
            name: "test".to_string(),
            address: "".to_string(), // Invalid
        };

        let result = coordinator.add_saga(config).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_repair_event_book_complete_passes_through() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_complete_event_book();

        let result = coordinator.repairer.repair_event_book(event_book.clone()).await;

        assert!(result.is_ok());
        let repaired = result.unwrap();
        assert_eq!(repaired.pages.len(), event_book.pages.len());
    }

    #[tokio::test]
    async fn test_repair_event_book_incomplete_without_repairer_warns() {
        let coordinator = SagaCoordinatorService::new();
        let event_book = make_incomplete_event_book();

        // Without a repairer, incomplete books pass through with a warning
        let result = coordinator.repairer.repair_event_book(event_book.clone()).await;

        assert!(result.is_ok());
        let passed = result.unwrap();
        // Still incomplete since no repairer
        assert_eq!(passed.pages.len(), 1);
    }

    #[test]
    fn test_extract_triggering_sequence_empty() {
        let book = make_event_book();
        assert_eq!(extract_triggering_sequence(&book), 0);
    }

    #[test]
    fn test_extract_triggering_sequence_single_event() {
        let book = make_complete_event_book();
        assert_eq!(extract_triggering_sequence(&book), 0);
    }

    #[test]
    fn test_extract_triggering_sequence_multiple_events() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![
                EventPage {
                    sequence: Some(event_page::Sequence::Num(0)),
                    event: Some(Any {
                        type_url: "test.Event".to_string(),
                        value: vec![],
                    }),
                    created_at: None,
                },
                EventPage {
                    sequence: Some(event_page::Sequence::Num(1)),
                    event: Some(Any {
                        type_url: "test.Event".to_string(),
                        value: vec![],
                    }),
                    created_at: None,
                },
                EventPage {
                    sequence: Some(event_page::Sequence::Num(5)),
                    event: Some(Any {
                        type_url: "test.Event".to_string(),
                        value: vec![],
                    }),
                    created_at: None,
                },
            ],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        assert_eq!(extract_triggering_sequence(&book), 5);
    }

    #[test]
    fn test_build_saga_origin_success() {
        let book = make_complete_event_book();
        let origin = build_saga_origin(&book, "loyalty_points");

        assert!(origin.is_some());
        let origin = origin.unwrap();
        assert_eq!(origin.saga_name, "loyalty_points");
        assert!(origin.triggering_aggregate.is_some());
        let agg = origin.triggering_aggregate.unwrap();
        assert_eq!(agg.domain, "orders");
        assert_eq!(origin.triggering_event_sequence, 0);
    }

    #[test]
    fn test_build_saga_origin_no_cover() {
        let book = EventBook {
            cover: None,
            pages: vec![],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let origin = build_saga_origin(&book, "test_saga");
        assert!(origin.is_none());
    }

    #[test]
    fn test_build_saga_origin_uses_max_sequence() {
        let book = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid { value: vec![1; 16] }),
            }),
            pages: vec![
                EventPage {
                    sequence: Some(event_page::Sequence::Num(3)),
                    event: None,
                    created_at: None,
                },
                EventPage {
                    sequence: Some(event_page::Sequence::Num(7)),
                    event: None,
                    created_at: None,
                },
            ],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let origin = build_saga_origin(&book, "test").unwrap();
        assert_eq!(origin.triggering_event_sequence, 7);
    }
}
