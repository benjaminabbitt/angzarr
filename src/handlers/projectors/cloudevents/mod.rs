//! CloudEvents projector support.
//!
//! Enables client projectors to emit CloudEvents for external consumption.
//! Client projectors return `Projection` with `CloudEventsResponse` packed
//! in the `projection` field. The coordinator detects this and routes to
//! configured sinks (HTTP webhooks, Kafka).
//!
//! # Architecture
//!
//! ```text
//!                     Client Code                        Framework
//!                     ───────────                        ─────────
//! EventBook ──→ [ProjectorHandler] ──→ Projection ──→ [CloudEventsCoordinator] ──→ Sink
//!               (existing client libs)  (CloudEventsResponse           (HTTP/Kafka)
//!                                        packed in .projection)
//! ```
//!
//! # Client Usage
//!
//! Clients use existing projector libraries (`ProjectorHandler`, `ProjectorBase`)
//! and pack a `CloudEventsResponse` into `Projection.projection`:
//!
//! ```ignore
//! fn transform_events(event_book: &EventBook) -> Projection {
//!     let mut cloud_events = Vec::new();
//!
//!     for page in &event_book.pages {
//!         if let Some(event) = &page.event {
//!             // Create filtered public event
//!             let public = PublicOrderCreated { order_id: "123" };
//!             let data = Any::from_msg(&public).unwrap();
//!
//!             cloud_events.push(CloudEvent {
//!                 r#type: "com.example.order.created".to_string(),
//!                 data: Some(data),
//!                 ..Default::default()
//!             });
//!         }
//!     }
//!
//!     let response = CloudEventsResponse { events: cloud_events };
//!     let projection_any = Any::from_msg(&response).unwrap();
//!
//!     Projection {
//!         cover: event_book.cover.clone(),
//!         projector: "prj-orders-cloudevents".to_string(),
//!         projection: Some(projection_any),
//!         ..Default::default()
//!     }
//! }
//! ```
//!
//! # Configuration
//!
//! | Variable | Description | Default |
//! |----------|-------------|---------|
//! | `CLOUDEVENTS_SINK` | `http`, `kafka`, or `both` | `http` |
//! | `CLOUDEVENTS_HTTP_ENDPOINT` | Webhook URL | required if http |
//! | `CLOUDEVENTS_KAFKA_BROKERS` | Broker list | required if kafka |
//! | `CLOUDEVENTS_KAFKA_TOPIC` | Topic name | `cloudevents` |

mod coordinator;
mod http_sink;
pub mod sink;
pub mod types;

#[cfg(feature = "kafka")]
mod kafka_sink;

pub use coordinator::CloudEventsCoordinator;
pub use http_sink::{HttpSink, HttpSinkConfig};
pub use sink::{CloudEventsSink, MultiSink, NullSink, SinkError};
pub use types::CloudEventEnvelope;

#[cfg(feature = "kafka")]
pub use kafka_sink::{KafkaSink, KafkaSinkConfig};

use std::sync::Arc;

/// Sink type to use for CloudEvents output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkType {
    /// HTTP webhook.
    Http,
    /// Kafka topic.
    #[cfg(feature = "kafka")]
    Kafka,
    /// Both HTTP and Kafka.
    #[cfg(feature = "kafka")]
    Both,
    /// No sink (discard events).
    Null,
}

impl SinkType {
    /// Parse from string (used for env var).
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "http" => Self::Http,
            #[cfg(feature = "kafka")]
            "kafka" => Self::Kafka,
            #[cfg(feature = "kafka")]
            "both" => Self::Both,
            "null" | "none" => Self::Null,
            _ => Self::Http, // Default
        }
    }

    /// Get from environment variable.
    pub fn from_env() -> Self {
        std::env::var("CLOUDEVENTS_SINK")
            .map(|s| Self::parse(&s))
            .unwrap_or(Self::Http)
    }
}

/// Create a sink from environment configuration.
///
/// Reads `CLOUDEVENTS_SINK` to determine sink type, then loads
/// type-specific configuration from other env vars.
pub fn sink_from_env() -> Result<Arc<dyn CloudEventsSink>, SinkError> {
    let sink_type = SinkType::from_env();

    match sink_type {
        SinkType::Http => {
            let sink = HttpSink::from_env()?;
            Ok(Arc::new(sink))
        }
        #[cfg(feature = "kafka")]
        SinkType::Kafka => {
            let sink = KafkaSink::from_env()?;
            Ok(Arc::new(sink))
        }
        #[cfg(feature = "kafka")]
        SinkType::Both => {
            let http = HttpSink::from_env()?;
            let kafka = KafkaSink::from_env()?;
            let multi = MultiSink::new(vec![
                Arc::new(http) as Arc<dyn CloudEventsSink>,
                Arc::new(kafka) as Arc<dyn CloudEventsSink>,
            ]);
            Ok(Arc::new(multi))
        }
        SinkType::Null => Ok(Arc::new(NullSink)),
    }
}
