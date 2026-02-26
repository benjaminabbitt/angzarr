//! Logging projector handler.
//!
//! Pretty-prints events to stdout with optional JSON decoding via prost-reflect.
//! If a `DESCRIPTOR_PATH` environment variable is set, events are decoded to JSON.
//! Otherwise, events are displayed as hex dumps.
//!
//! # Output Abstraction
//!
//! The service uses a [`LogOutput`] trait for output, allowing different destinations:
//! - [`StdoutOutput`] - Plain stdout
//! - [`FileOutput`] - File output
//! - [`ColorizingOutput`] - Decorator that adds ANSI colors
//!
//! By default, uses [`ColorizingOutput`] wrapping [`StdoutOutput`] with pattern-based
//! color classification.

use std::sync::Arc;

use prost_reflect::{DescriptorPool, DynamicMessage};
use tonic::{Request, Response, Status};
use tracing::info;

use super::output::{ColorizingOutput, DecodedEvent, EventColorConfig, LogOutput};
use crate::proto::projector_coordinator_service_server::ProjectorCoordinatorService;
use crate::proto::{EventBook, EventRequest, Projection, SpeculateProjectorRequest};
use crate::proto_ext::EventPageExt;

/// Logging projector service.
///
/// Receives events and pretty-prints them using a configurable output.
/// If `DESCRIPTOR_PATH` is set, decodes protobuf messages to JSON.
pub struct LogService {
    pool: Option<DescriptorPool>,
    output: Box<dyn LogOutput>,
}

impl LogService {
    /// Create a new logging service with default colorized stdout output.
    ///
    /// Attempts to load a `FileDescriptorSet` from `DESCRIPTOR_PATH` if set.
    pub fn new() -> Self {
        Self::with_output(Box::new(ColorizingOutput::with_default_patterns()))
    }

    /// Create a new logging service with custom output.
    pub fn with_output(output: Box<dyn LogOutput>) -> Self {
        let pool = std::env::var("DESCRIPTOR_PATH").ok().and_then(|path| {
            info!(path = %path, "Loading protobuf descriptors");
            let bytes = std::fs::read(&path).ok()?;
            match DescriptorPool::decode(bytes.as_slice()) {
                Ok(pool) => {
                    info!(
                        message_count = pool.all_messages().count(),
                        "Loaded protobuf descriptors"
                    );
                    Some(pool)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to decode descriptor set");
                    None
                }
            }
        });

        if pool.is_none() {
            info!("No DESCRIPTOR_PATH set - events will be displayed as hex");
        }

        Self { pool, output }
    }

    /// Create a new logging service with custom output and color config.
    pub fn with_color_config(config: EventColorConfig) -> Self {
        Self::with_output(Box::new(ColorizingOutput::new(config)))
    }

    /// Decode an Any message to a pretty-printed string.
    fn decode_event(&self, any: &prost_types::Any) -> String {
        // Strip "type.googleapis.com/" or similar prefix from type_url
        let type_name = any.type_url.rsplit('/').next().unwrap_or(&any.type_url);

        if let Some(pool) = &self.pool {
            if let Some(desc) = pool.get_message_by_name(type_name) {
                if let Ok(msg) = DynamicMessage::decode(desc, any.value.as_slice()) {
                    return serde_json::to_string_pretty(&msg)
                        .unwrap_or_else(|_| self.hex_dump(&any.value));
                }
            }
        }

        // Fallback: hex dump for unknown types
        self.hex_dump(&any.value)
    }

    /// Format bytes as a hex dump.
    fn hex_dump(&self, bytes: &[u8]) -> String {
        let preview_len = 64.min(bytes.len());
        let hex = hex::encode(&bytes[..preview_len]);
        if bytes.len() > preview_len {
            format!("<{} bytes: {}...>", bytes.len(), hex)
        } else {
            format!("<{} bytes: {}>", bytes.len(), hex)
        }
    }

    /// Handle an event book by logging all events.
    pub fn handle(&self, book: &EventBook) {
        let cover = match &book.cover {
            Some(c) => c,
            None => {
                tracing::warn!("EventBook missing cover");
                return;
            }
        };

        let root_id = cover
            .root
            .as_ref()
            .map(|u| hex::encode(&u.value[..8.min(u.value.len())]))
            .unwrap_or_else(|| "unknown".to_string());

        for page in &book.pages {
            let sequence = page.sequence_num();

            let Some(crate::proto::event_page::Payload::Event(event)) = &page.payload else {
                continue;
            };

            // Extract full type name for reflection-based classification
            let type_name = event.type_url.rsplit('/').next().unwrap_or(&event.type_url);
            let content = self.decode_event(event);

            let decoded = DecodedEvent {
                domain: &cover.domain,
                root_id: &root_id,
                sequence,
                type_name,
                content: &content,
            };

            self.output.write_event(&decoded);
        }
    }
}

impl Default for LogService {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl ProjectorCoordinatorService for LogService {
    async fn handle_sync(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<Projection>, Status> {
        if let Some(book) = request.into_inner().events {
            self.handle(&book);
        }
        Ok(Response::new(Projection::default()))
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let book = request.into_inner();
        self.handle(&book);
        Ok(Response::new(()))
    }

    async fn handle_speculative(
        &self,
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        if let Some(book) = request.into_inner().events {
            self.handle(&book);
        }
        Ok(Response::new(Projection::default()))
    }
}

/// Wrapper to share LogService across async contexts.
#[derive(Clone)]
pub struct LogServiceHandle(pub Arc<LogService>);

impl std::ops::Deref for LogServiceHandle {
    type Target = LogService;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tonic::async_trait]
impl ProjectorCoordinatorService for LogServiceHandle {
    async fn handle_sync(
        &self,
        request: Request<EventRequest>,
    ) -> Result<Response<Projection>, Status> {
        ProjectorCoordinatorService::handle_sync(&*self.0, request).await
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        ProjectorCoordinatorService::handle(&*self.0, request).await
    }

    async fn handle_speculative(
        &self,
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        ProjectorCoordinatorService::handle_speculative(&*self.0, request).await
    }
}
