//! Saga coordinator service.
//!
//! Receives events from the event bus and distributes them to registered sagas.
//! Sagas can produce new commands in response to events, enabling cross-aggregate
//! workflows. Ensures sagas receive complete EventBooks by fetching missing history
//! from the EventQuery service when needed.

use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
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
    repairer: Arc<Mutex<EventBookRepairer>>,
}

impl SagaCoordinatorService {
    /// Create a new saga coordinator.
    pub fn new(repairer: EventBookRepairer) -> Self {
        Self {
            sagas: Arc::new(RwLock::new(Vec::new())),
            repairer: Arc::new(Mutex::new(repairer)),
        }
    }

    /// Create a new saga coordinator, connecting to EventQuery service.
    pub async fn connect(event_query_address: &str) -> Result<Self, String> {
        let repairer = EventBookRepairer::connect(event_query_address)
            .await
            .map_err(|e| format!("Failed to connect to EventQuery service: {}", e))?;

        info!(
            address = %event_query_address,
            "Connected to EventQuery service for EventBook repair"
        );

        Ok(Self::new(repairer))
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
        let event_book = self.repairer.lock().await.repair(event_book).await.map_err(|e| {
            error!(error = %e, "Failed to repair EventBook");
            Status::internal(format!("Failed to repair EventBook: {}", e))
        })?;

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
        let event_book = self.repairer.lock().await.repair(event_book).await.map_err(|e| {
            error!(error = %e, "Failed to repair EventBook");
            Status::internal(format!("Failed to repair EventBook: {}", e))
        })?;

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
