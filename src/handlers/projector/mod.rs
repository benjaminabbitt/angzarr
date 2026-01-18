//! Projector event handler for projector sidecar.
//!
//! Receives events from the event bus (AMQP) and forwards them to projector
//! coordinator services. Ensures projectors receive complete EventBooks by
//! fetching missing history from the EventQuery service when needed.
//!
//! When projectors produce output (Projections), these are published back
//! to AMQP as synthetic EventBooks with the original correlation_id preserved,
//! enabling streaming of projector results back to clients via angzarr-stream.

use std::sync::Arc;

use futures::future::BoxFuture;
use prost::Message;
use prost_types::Any;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::bus::{AmqpEventBus, BusError, EventBus, EventHandler};
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::{EventBook, Projection, SyncEventBook, SyncMode};
use crate::services::event_book_repair::{self, EventBookRepairer};

/// Event handler that forwards events to a projector gRPC service.
///
/// Calls `handle_sync` to get projector output, then publishes the
/// Projection back to AMQP as a synthetic EventBook for streaming.
pub struct ProjectorEventHandler {
    client: Arc<Mutex<ProjectorCoordinatorClient<tonic::transport::Channel>>>,
    publisher: Option<Arc<AmqpEventBus>>,
    repairer: Option<Arc<Mutex<EventBookRepairer>>>,
}

impl ProjectorEventHandler {
    /// Create a new projector event handler without streaming output.
    ///
    /// Projector results are not published back to AMQP.
    pub fn new(client: ProjectorCoordinatorClient<tonic::transport::Channel>) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            publisher: None,
            repairer: None,
        }
    }

    /// Create a new projector event handler with streaming output.
    ///
    /// Projector results are published back to AMQP for streaming to clients.
    pub fn with_publisher(
        client: ProjectorCoordinatorClient<tonic::transport::Channel>,
        publisher: AmqpEventBus,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            publisher: Some(Arc::new(publisher)),
            repairer: None,
        }
    }

    /// Create a new projector event handler with repair and streaming.
    pub fn with_repairer_and_publisher(
        client: ProjectorCoordinatorClient<tonic::transport::Channel>,
        repairer: EventBookRepairer,
        publisher: AmqpEventBus,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            publisher: Some(Arc::new(publisher)),
            repairer: Some(Arc::new(Mutex::new(repairer))),
        }
    }

    /// Create a new projector event handler with repair capability only.
    pub fn with_repairer(
        client: ProjectorCoordinatorClient<tonic::transport::Channel>,
        repairer: EventBookRepairer,
    ) -> Self {
        Self {
            client: Arc::new(Mutex::new(client)),
            publisher: None,
            repairer: Some(Arc::new(Mutex::new(repairer))),
        }
    }
}

impl EventHandler for ProjectorEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        let client = self.client.clone();
        let publisher = self.publisher.clone();
        let repairer = self.repairer.clone();

        Box::pin(async move {
            let book_owned = (*book).clone();
            let correlation_id = book_owned.correlation_id.clone();

            // Repair if incomplete
            let book_to_send = if event_book_repair::is_complete(&book_owned) {
                book_owned
            } else if let Some(ref repairer) = repairer {
                let mut repairer = repairer.lock().await;
                repairer.repair(book_owned).await.map_err(|e| {
                    error!(error = %e, "Failed to repair EventBook");
                    BusError::Grpc(tonic::Status::internal(format!(
                        "Failed to repair EventBook: {}",
                        e
                    )))
                })?
            } else {
                warn!(
                    "Received incomplete EventBook but no repairer configured, passing through as-is"
                );
                book_owned
            };

            // Call projector coordinator handle_sync to get the Projection result
            let mut client = client.lock().await;
            let sync_request = SyncEventBook {
                events: Some(book_to_send.clone()),
                sync_mode: SyncMode::Simple.into(),
            };
            let response = client
                .handle_sync(sync_request)
                .await
                .map_err(BusError::Grpc)?;
            let projection = response.into_inner();

            // If we have a publisher and the projection has content, publish it back
            if let Some(ref publisher) = publisher {
                // Only publish if projection has actual content
                if projection.projection.is_some() || !projection.projector.is_empty() {
                    debug!(
                        correlation_id = %correlation_id,
                        projector = %projection.projector,
                        sequence = projection.sequence,
                        "Publishing projection output"
                    );

                    let projection_event_book =
                        create_projection_event_book(projection, &correlation_id);

                    info!(
                        correlation_id = %correlation_id,
                        domain = projection_event_book.cover.as_ref().map(|c| c.domain.as_str()).unwrap_or("unknown"),
                        "Publishing projection to AMQP for streaming"
                    );

                    publisher.publish(Arc::new(projection_event_book)).await?;
                }
            }

            Ok(())
        })
    }
}

/// Convert a Projection to a synthetic EventBook for AMQP transport.
///
/// Uses a special domain prefix `_projection.{projector_name}` so clients
/// can distinguish projection results from domain events. The projection
/// is serialized as the event payload - clients deserialize the Projection
/// proto from the event.
fn create_projection_event_book(projection: Projection, correlation_id: &str) -> EventBook {
    let projector_name = projection.projector.clone();

    // Create a cover with special projection domain
    let cover = projection.cover.clone().map(|mut c| {
        c.domain = format!("_projection.{}.{}", projector_name, c.domain);
        c
    });

    // Serialize the projection as the event payload
    let projection_bytes = projection.encode_to_vec();

    EventBook {
        cover,
        // No snapshot - snapshots are aggregate state, not projection transport
        snapshot: None,
        pages: vec![crate::proto::EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(projection.sequence)),
            event: Some(Any {
                type_url: "angzarr.Projection".to_string(),
                value: projection_bytes,
            }),
            created_at: None,
        }],
        correlation_id: correlation_id.to_string(),
        snapshot_state: None,
    }
}
