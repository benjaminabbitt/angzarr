//! Projector event handler.
//!
//! Receives events from the event bus and forwards them to projector
//! services via the `ProjectorContext` abstraction.
//!
//! Works with any `ProjectorContext` implementation — gRPC (distributed)
//! or local (standalone) — enabling deploy-anywhere projector code.
//!
//! When projectors produce output (Projections), these are published back
//! to the event bus as synthetic EventBooks with the original correlation_id
//! preserved, enabling streaming of projector results back to clients via
//! angzarr-stream.

use std::sync::Arc;

use futures::future::BoxFuture;
use prost::Message;
use prost_types::Any;
use tokio::sync::Mutex;
use tracing::{debug, info, Instrument};

use crate::bus::{BusError, EventBus, EventHandler};
use crate::orchestration::projector::grpc::GrpcProjectorContext;
use crate::orchestration::projector::ProjectorContext;
use crate::proto::projector_coordinator_client::ProjectorCoordinatorClient;
use crate::proto::{EventBook, Projection};
use crate::proto_ext::CoverExt;

/// Event handler that forwards events to a projector via context abstraction.
///
/// Uses `ProjectorContext` for the actual projector call, enabling the same
/// handler code for both distributed (gRPC) and standalone (local) modes.
///
/// Calls projector to get output, then publishes the Projection back to
/// the event bus as a synthetic EventBook for streaming.
pub struct ProjectorEventHandler {
    context: Arc<dyn ProjectorContext>,
    publisher: Option<Arc<dyn EventBus>>,
    /// Domain filter — only handle events from these domains. Empty = all.
    domains: Vec<String>,
    /// If true, this projector is synchronous (handled inline by the aggregate pipeline).
    /// Async distribution should skip it.
    synchronous: bool,
    /// Projector name (used for metrics and tracing).
    name: String,
}

impl ProjectorEventHandler {
    /// Create from a projector context without streaming output.
    pub fn from_context(context: Arc<dyn ProjectorContext>, name: String) -> Self {
        Self {
            context,
            publisher: None,
            domains: Vec::new(),
            synchronous: false,
            name,
        }
    }

    /// Create from a projector context with streaming output.
    pub fn from_context_with_publisher(
        context: Arc<dyn ProjectorContext>,
        publisher: Arc<dyn EventBus>,
        name: String,
    ) -> Self {
        Self {
            context,
            publisher: Some(publisher),
            domains: Vec::new(),
            synchronous: false,
            name,
        }
    }

    /// Create with full configuration including domain filtering and sync flag.
    pub fn with_config(
        context: Arc<dyn ProjectorContext>,
        publisher: Option<Arc<dyn EventBus>>,
        domains: Vec<String>,
        synchronous: bool,
        name: String,
    ) -> Self {
        Self {
            context,
            publisher,
            domains,
            synchronous,
            name,
        }
    }

    // --- Backward-compatible constructors for distributed sidecar binaries ---

    /// Create a new projector event handler without streaming output.
    pub fn new(
        client: ProjectorCoordinatorClient<tonic::transport::Channel>,
        name: String,
    ) -> Self {
        let context = Arc::new(GrpcProjectorContext::new(Arc::new(Mutex::new(client))));
        Self {
            context,
            publisher: None,
            domains: Vec::new(),
            synchronous: false,
            name,
        }
    }

    /// Create a new projector event handler with streaming output.
    pub fn with_publisher(
        client: ProjectorCoordinatorClient<tonic::transport::Channel>,
        publisher: Arc<dyn EventBus>,
        name: String,
    ) -> Self {
        let context = Arc::new(GrpcProjectorContext::new(Arc::new(Mutex::new(client))));
        Self {
            context,
            publisher: Some(publisher),
            domains: Vec::new(),
            synchronous: false,
            name,
        }
    }
}

impl EventHandler for ProjectorEventHandler {
    fn handle(&self, book: Arc<EventBook>) -> BoxFuture<'static, Result<(), BusError>> {
        // Skip synchronous projectors in async distribution
        if self.synchronous {
            return Box::pin(async { Ok(()) });
        }

        // Check domain filter
        if !self.domains.is_empty() {
            let domain = book.domain();
            if !self.domains.iter().any(|d| d == domain) {
                return Box::pin(async { Ok(()) });
            }
        }

        let correlation_id = book
            .cover
            .as_ref()
            .map(|c| c.correlation_id.clone())
            .unwrap_or_default();
        let domain = book.domain().to_string();
        let projector_name = self.name.clone();
        let span = tracing::info_span!("projector.handle", %projector_name, %correlation_id, %domain);

        let context = self.context.clone();
        let publisher = self.publisher.clone();

        Box::pin(async move {
            #[cfg(feature = "otel")]
            let start = std::time::Instant::now();

            let book_owned = (*book).clone();

            let result: Result<(), BusError> = async {
                let projection = context
                    .handle_events(&book_owned)
                    .await
                    .map_err(BusError::Grpc)?;

                // If we have a publisher and the projection has content, publish it back
                if let Some(ref publisher) = publisher {
                    if projection.projection.is_some() || !projection.projector.is_empty() {
                        debug!(
                            projector = %projection.projector,
                            sequence = projection.sequence,
                            "Publishing projection output"
                        );

                        let projection_event_book =
                            create_projection_event_book(projection, &correlation_id);

                        info!(
                            domain = %projection_event_book.domain(),
                            "Publishing projection for streaming"
                        );

                        publisher.publish(Arc::new(projection_event_book)).await?;
                    }
                }

                Ok(())
            }
            .await;

            #[cfg(feature = "otel")]
            {
                use crate::utils::metrics::{self, PROJECTOR_DURATION};
                PROJECTOR_DURATION.record(start.elapsed().as_secs_f64(), &[
                    metrics::component_attr("projector"),
                    metrics::name_attr(&projector_name),
                    metrics::domain_attr(&domain),
                ]);
            }

            result
        }.instrument(span))
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

    // Ensure correlation_id is set on cover
    let cover = match cover {
        Some(mut c) => {
            if c.correlation_id.is_empty() {
                c.correlation_id = correlation_id.to_string();
            }
            Some(c)
        }
        None => Some(crate::proto::Cover {
            domain: format!("_projection.{}", projector_name),
            root: None,
            correlation_id: correlation_id.to_string(),
            edition: None,
        }),
    };

    EventBook {
        cover,
        snapshot: None,
        pages: vec![crate::proto::EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(projection.sequence)),
            event: Some(Any {
                type_url: "angzarr.Projection".to_string(),
                value: projection_bytes,
            }),
            created_at: None,
        }],
        snapshot_state: None,
    }
}
