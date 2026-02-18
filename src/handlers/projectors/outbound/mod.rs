//! Unified outbound projector for event streaming.
//!
//! Merges gRPC streaming (internal consumers) with CloudEvents sinks (external consumers).
//!
//! # Architecture
//!
//! ```text
//!                                     ┌── gRPC stream (EventBook, multi-page)
//! EventBook ──→ [OutboundService] ────┼── HTTP (CloudEvents JSON/proto)
//!                                     └── Kafka (CloudEvents JSON/proto)
//! ```
//!
//! ## CloudEvents Mapping
//!
//! Each CloudEvent contains an **EventBook with a single page**:
//! - Multi-page EventBook → N CloudEvents, each with 1-page EventBook as `data`
//! - Preserves EventBook structure (Cover, metadata)
//! - Granular for external consumers
//!
//! ## Dual Protocol Support
//!
//! - **gRPC stream**: Internal consumers receive EventBooks (multi-page) via gRPC streaming
//! - **CloudEvents**: External consumers receive CloudEvents over HTTP/Kafka (JSON or protobuf)
//!
//! # Configuration
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `OUTBOUND_SINKS` | Comma-separated: `http,kafka` | none |
//! | `OUTBOUND_CONTENT_TYPE` | `json` or `protobuf` | `json` |
//! | `CLOUDEVENTS_HTTP_ENDPOINT` | HTTP webhook URL | (required if http) |
//! | `CLOUDEVENTS_KAFKA_BROKERS` | Kafka brokers | (required if kafka) |

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use cloudevents::{EventBuilder, EventBuilderV10};
use futures::Stream;
use prost::Message;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::bus::{BusError, EventHandler};
use crate::proto::event_stream_service_server::EventStreamService;
use crate::proto::{CloudEventsResponse, EventBook, EventStreamFilter, Projection};

use super::cloudevents::sink::{CloudEventsSink, SinkError};
use super::cloudevents::types::{CloudEventEnvelope, ContentType};

type SubscriberSender = mpsc::Sender<Result<EventBook, Status>>;

/// Subscriber registration for gRPC streaming.
pub(crate) struct Subscriber {
    pub(crate) sender: SubscriberSender,
}

/// Unified outbound service for event streaming.
///
/// Combines:
/// - gRPC streaming for internal consumers (EventBook format)
/// - CloudEvents HTTP/Kafka for external consumers
pub struct OutboundService {
    /// Map of correlation_id -> list of gRPC subscribers.
    subscriptions: Arc<RwLock<HashMap<String, Vec<Subscriber>>>>,
    /// CloudEvents sinks for external consumers.
    sinks: Vec<Arc<dyn CloudEventsSink>>,
    /// Content type for CloudEvents serialization.
    content_type: ContentType,
}

impl OutboundService {
    /// Create a new outbound service with no sinks (gRPC only).
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            sinks: Vec::new(),
            content_type: ContentType::default(),
        }
    }

    /// Create with CloudEvents sinks.
    pub fn with_sinks(sinks: Vec<Arc<dyn CloudEventsSink>>) -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            sinks,
            content_type: ContentType::default(),
        }
    }

    /// Set content type for CloudEvents.
    pub fn with_content_type(mut self, content_type: ContentType) -> Self {
        self.content_type = content_type;
        self
    }

    /// Get a reference to subscriptions for the event handler.
    pub(crate) fn subscriptions(&self) -> Arc<RwLock<HashMap<String, Vec<Subscriber>>>> {
        Arc::clone(&self.subscriptions)
    }

    /// Process an event book - forward to gRPC subscribers and CloudEvents sinks.
    ///
    /// This is the main entry point for handling events.
    pub async fn handle(&self, book: &EventBook) -> Result<(), SinkError> {
        // 1. Forward to gRPC subscribers (EventBook, multi-page)
        self.forward_to_grpc_subscribers(book).await;

        // 2. Publish to CloudEvents sinks if configured
        if !self.sinks.is_empty() {
            self.publish_to_sinks(book).await?;
        }

        Ok(())
    }

    /// Process a CloudEventsResponse from a client projector.
    ///
    /// This handles the case where a client projector returns CloudEventsResponse
    /// packed in a Projection. Used for custom client-side event transformation.
    ///
    /// Returns true if the projection was a CloudEventsResponse and was processed.
    pub async fn process_projection(
        &self,
        projection: &Projection,
        source_events: Option<&EventBook>,
    ) -> Result<bool, SinkError> {
        // Check if projection contains CloudEventsResponse
        let Some(projection_any) = &projection.projection else {
            return Ok(false);
        };

        // Check type_url for CloudEventsResponse
        if !projection_any.type_url.ends_with("CloudEventsResponse") {
            return Ok(false);
        }

        // Decode CloudEventsResponse
        let response = match CloudEventsResponse::decode(&projection_any.value[..]) {
            Ok(r) => r,
            Err(e) => {
                error!(
                    projector = %projection.projector,
                    error = %e,
                    "Failed to decode CloudEventsResponse"
                );
                return Err(SinkError::Serialization(serde_json::Error::io(
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
                )));
            }
        };

        if response.events.is_empty() {
            debug!(
                projector = %projection.projector,
                "CloudEventsResponse empty, skipping"
            );
            return Ok(true);
        }

        // Convert to CloudEvents SDK events
        let envelopes =
            convert_proto_events(&response.events, projection, source_events, self.content_type)?;

        // Publish to sinks
        for sink in &self.sinks {
            sink.publish(envelopes.clone(), self.content_type).await?;
        }

        debug!(
            projector = %projection.projector,
            event_count = response.events.len(),
            "CloudEvents from projector published"
        );

        Ok(true)
    }

    /// Forward event book to gRPC subscribers.
    async fn forward_to_grpc_subscribers(&self, book: &EventBook) {
        // Skip events without correlation ID for gRPC streaming
        let correlation_id = match book.cover.as_ref() {
            Some(c) if !c.correlation_id.is_empty() => &c.correlation_id,
            _ => {
                debug!("No correlation_id for gRPC streaming, skipping gRPC forward");
                return;
            }
        };

        let mut subs = self.subscriptions.write().await;
        if let Some(subscribers) = subs.get_mut(correlation_id) {
            let mut to_remove = Vec::new();
            let mut sent_count = 0;

            for (idx, sub) in subscribers.iter().enumerate() {
                if sub.sender.is_closed() {
                    to_remove.push(idx);
                    continue;
                }

                if let Err(e) = sub.sender.try_send(Ok(book.clone())) {
                    warn!(
                        correlation_id = %correlation_id,
                        error = %e,
                        "Failed to send event to gRPC subscriber"
                    );
                    if sub.sender.is_closed() {
                        to_remove.push(idx);
                    }
                } else {
                    sent_count += 1;
                }
            }

            // Remove closed subscribers (reverse order to preserve indices)
            if !to_remove.is_empty() {
                debug!(
                    correlation_id = %correlation_id,
                    removed = to_remove.len(),
                    "Removing disconnected gRPC subscribers"
                );
                for idx in to_remove.into_iter().rev() {
                    subscribers.remove(idx);
                }
            }

            // Remove correlation_id entry if no subscribers left
            if subscribers.is_empty() {
                subs.remove(correlation_id);
                debug!(
                    correlation_id = %correlation_id,
                    "No gRPC subscribers remaining, removed correlation entry"
                );
            } else {
                debug!(
                    correlation_id = %correlation_id,
                    sent = sent_count,
                    remaining = subscribers.len(),
                    "Event delivered to gRPC subscribers"
                );
            }
        }
    }

    /// Publish event book to CloudEvents sinks.
    ///
    /// Splits multi-page EventBook into single-page EventBooks,
    /// wraps each as a CloudEvent.
    async fn publish_to_sinks(&self, book: &EventBook) -> Result<(), SinkError> {
        if book.pages.is_empty() {
            return Ok(());
        }

        // Build CloudEvents from pages
        let events: Vec<CloudEventEnvelope> = book
            .pages
            .iter()
            .enumerate()
            .filter_map(|(idx, page)| {
                // Create single-page EventBook
                let single_page_book = EventBook {
                    cover: book.cover.clone(),
                    pages: vec![page.clone()],
                    snapshot: None,
                    ..Default::default()
                };

                wrap_eventbook_as_cloudevent(&single_page_book, idx).ok()
            })
            .collect();

        if events.is_empty() {
            return Ok(());
        }

        // Publish to all sinks
        for sink in &self.sinks {
            if let Err(e) = sink.publish(events.clone(), self.content_type).await {
                error!(
                    sink = %sink.name(),
                    error = %e,
                    "Failed to publish to CloudEvents sink"
                );
                // Continue with other sinks
            }
        }

        debug!(
            event_count = events.len(),
            sink_count = self.sinks.len(),
            "EventBook published to CloudEvents sinks"
        );

        Ok(())
    }
}

impl Default for OutboundService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl EventStreamService for OutboundService {
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<EventBook, Status>> + Send + 'static>>;

    async fn subscribe(
        &self,
        request: Request<EventStreamFilter>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let filter = request.into_inner();

        if filter.correlation_id.is_empty() {
            return Err(Status::invalid_argument(
                "correlation_id is required for event stream subscription",
            ));
        }

        let correlation_id = filter.correlation_id.clone();
        debug!(correlation_id = %correlation_id, "New gRPC subscription registered");

        // Create channel for this subscriber
        let (tx, rx) = mpsc::channel(32);

        // Clone tx for cleanup task before moving into subscriber
        let cleanup_tx = tx.clone();

        // Register subscriber
        {
            let mut subs = self.subscriptions.write().await;
            subs.entry(correlation_id.clone())
                .or_default()
                .push(Subscriber { sender: tx });
        }

        // Spawn cleanup task that removes subscriber when channel closes
        let subscriptions = Arc::clone(&self.subscriptions);
        let cleanup_correlation_id = correlation_id.clone();
        tokio::spawn(async move {
            cleanup_tx.closed().await;
            info!(
                correlation_id = %cleanup_correlation_id,
                "gRPC subscriber disconnected, cleaning up subscription"
            );
            let mut subs = subscriptions.write().await;
            if let Some(subscribers) = subs.get_mut(&cleanup_correlation_id) {
                let before_count = subscribers.len();
                subscribers.retain(|s| !s.sender.is_closed());
                let removed_count = before_count - subscribers.len();
                if subscribers.is_empty() {
                    subs.remove(&cleanup_correlation_id);
                    debug!(
                        correlation_id = %cleanup_correlation_id,
                        "Removed last gRPC subscriber, correlation entry cleaned up"
                    );
                } else {
                    debug!(
                        correlation_id = %cleanup_correlation_id,
                        removed = removed_count,
                        remaining = subscribers.len(),
                        "Removed closed gRPC subscribers"
                    );
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

/// Event handler that forwards to outbound service.
pub struct OutboundEventHandler {
    service: Arc<OutboundService>,
}

impl OutboundEventHandler {
    /// Create handler from outbound service.
    pub fn new(service: Arc<OutboundService>) -> Self {
        Self { service }
    }
}

impl EventHandler for OutboundEventHandler {
    fn handle(
        &self,
        book: Arc<EventBook>,
    ) -> futures::future::BoxFuture<'static, Result<(), BusError>> {
        let service = Arc::clone(&self.service);

        Box::pin(async move {
            if let Err(e) = service.handle(&book).await {
                warn!(error = %e, "OutboundService handle failed");
            }
            Ok(())
        })
    }
}

/// Wrap a single-page EventBook as a CloudEvent.
fn wrap_eventbook_as_cloudevent(
    book: &EventBook,
    sequence_offset: usize,
) -> Result<CloudEventEnvelope, SinkError> {
    let cover = book.cover.as_ref();
    let domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
    let root_id = cover
        .and_then(|c| c.root.as_ref())
        .map(|u| hex::encode(&u.value))
        .unwrap_or_else(|| "unknown".to_string());
    let correlation_id = cover.map(|c| c.correlation_id.as_str()).unwrap_or("");

    // Get event type from page
    let page = book.pages.first();
    let event_type = page
        .and_then(|p| p.event.as_ref())
        .map(|e| {
            e.type_url
                .rsplit('/')
                .next()
                .unwrap_or(&e.type_url)
                .to_string()
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get sequence from page
    let sequence = page
        .and_then(|p| p.sequence.as_ref())
        .and_then(|s| match s {
            crate::proto::event_page::Sequence::Num(n) => Some(*n),
            crate::proto::event_page::Sequence::Force(_) => Some(0),
        })
        .unwrap_or(sequence_offset as u32);

    // Get timestamp from page
    let time = page
        .and_then(|p| p.created_at.as_ref())
        .and_then(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32))
        .unwrap_or_else(chrono::Utc::now);

    // Build CloudEvent with EventBook as data
    let id = format!("{}:{}:{}", domain, root_id, sequence);
    let source = format!("angzarr/{}", domain);

    let book_bytes = book.encode_to_vec();

    let mut builder = EventBuilderV10::new()
        .id(id)
        .ty(format!("angzarr.{}", event_type))
        .source(source)
        .time(time)
        .subject(root_id.clone())
        .data("application/x-protobuf", book_bytes);

    // Add correlation_id as extension if present
    if !correlation_id.is_empty() {
        builder = builder.extension("correlationid", correlation_id);
    }

    builder.build().map_err(|e| {
        error!(error = %e, "Failed to build CloudEvent from EventBook");
        SinkError::Config(format!("Invalid CloudEvent: {}", e))
    })
}

/// Convert proto CloudEvents to SDK Event objects (for client projector responses).
fn convert_proto_events(
    events: &[crate::proto::CloudEvent],
    projection: &Projection,
    source_events: Option<&EventBook>,
    _content_type: ContentType,
) -> Result<Vec<CloudEventEnvelope>, SinkError> {
    let cover = projection.cover.as_ref();
    let domain = cover.map(|c| c.domain.as_str()).unwrap_or("unknown");
    let root_id = cover
        .and_then(|c| c.root.as_ref())
        .map(|u| hex::encode(&u.value))
        .unwrap_or_else(|| "unknown".to_string());
    let correlation_id = cover.map(|c| c.correlation_id.as_str()).unwrap_or("");

    // Get timestamp from first source event if available
    let default_time = source_events
        .and_then(|e| e.pages.first())
        .and_then(|p| p.created_at.as_ref())
        .and_then(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32))
        .unwrap_or_else(chrono::Utc::now);

    let mut envelopes = Vec::with_capacity(events.len());

    for (idx, event) in events.iter().enumerate() {
        let envelope = convert_single_proto_event(
            event,
            domain,
            &root_id,
            correlation_id,
            default_time,
            projection.sequence.saturating_add(idx as u32),
        )?;
        envelopes.push(envelope);
    }

    Ok(envelopes)
}

/// Convert a single proto CloudEvent to SDK Event.
fn convert_single_proto_event(
    event: &crate::proto::CloudEvent,
    domain: &str,
    root_id: &str,
    correlation_id: &str,
    default_time: chrono::DateTime<chrono::Utc>,
    sequence: u32,
) -> Result<CloudEventEnvelope, SinkError> {
    use super::cloudevents::types::normalize_extension_key;

    // Use provided values or defaults
    let id = event
        .id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}:{}:{}", domain, root_id, sequence));

    let event_type = if event.r#type.is_empty() {
        // Derive from data type_url if available
        event
            .data
            .as_ref()
            .map(|d| {
                d.type_url
                    .rsplit('/')
                    .next()
                    .unwrap_or(&d.type_url)
                    .to_string()
            })
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        event.r#type.clone()
    };

    let source = event
        .source
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("angzarr/{}", domain));

    let subject = event
        .subject
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| root_id.to_string());

    // Convert data Any to JSON
    let data = event.data.as_ref().and_then(any_to_json);

    // Build event using SDK builder
    let mut builder = EventBuilderV10::new()
        .id(id)
        .ty(event_type)
        .source(source)
        .time(default_time)
        .subject(subject);

    // Set data if present
    if let Some(json_data) = data {
        builder = builder.data("application/json", json_data);
    }

    // Add correlation_id as extension if present (lowercase per spec)
    if !correlation_id.is_empty() {
        builder = builder.extension("correlationid", correlation_id);
    }

    // Add client-provided extensions (normalize keys to lowercase)
    for (key, value) in &event.extensions {
        let normalized_key = normalize_extension_key(key);
        builder = builder.extension(&normalized_key, value.as_str());
    }

    // Build and validate the event
    builder.build().map_err(|e| {
        error!(error = %e, "Failed to build CloudEvent");
        SinkError::Config(format!("Invalid CloudEvent: {}", e))
    })
}

/// Convert proto Any to JSON Value using prost-reflect.
fn any_to_json(any: &prost_types::Any) -> Option<serde_json::Value> {
    use crate::proto_reflect;

    // Try to decode using global descriptor pool
    match proto_reflect::decode_any(any) {
        Ok(msg) => match serde_json::to_value(&msg) {
            Ok(v) => Some(v),
            Err(e) => {
                warn!(
                    type_url = %any.type_url,
                    error = %e,
                    "Failed to serialize DynamicMessage to JSON"
                );
                // Fallback: base64 encode the binary
                Some(serde_json::json!({
                    "_type": any.type_url,
                    "_binary": base64_encode(&any.value),
                    "_size": any.value.len()
                }))
            }
        },
        Err(e) => {
            // Descriptor pool may not have this type
            debug!(
                type_url = %any.type_url,
                error = %e,
                "Proto type not in descriptor pool, using binary fallback"
            );
            // Fallback: base64 encode the binary
            Some(serde_json::json!({
                "_type": any.type_url,
                "_binary": base64_encode(&any.value),
                "_size": any.value.len()
            }))
        }
    }
}

/// Base64 encode bytes (standard alphabet).
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Create outbound service from environment configuration.
pub fn from_env() -> Result<OutboundService, SinkError> {
    use super::cloudevents::{sink_from_env, SinkType};

    let sink_type = SinkType::from_env();
    let content_type = std::env::var("OUTBOUND_CONTENT_TYPE")
        .map(|s| ContentType::parse(&s))
        .unwrap_or_default();

    let sinks = if sink_type == SinkType::Null {
        Vec::new()
    } else {
        vec![sink_from_env()?]
    };

    Ok(OutboundService::with_sinks(sinks).with_content_type(content_type))
}

#[cfg(test)]
mod tests;
