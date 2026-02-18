//! Projector implementations.
//!
//! Contains actual projector services that process events and produce output.
//! These implement the ProjectorCoordinator gRPC service.

pub mod cloudevents;
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod event;
pub mod log;
pub mod outbound;
pub mod stream;

pub use cloudevents::{
    CloudEventEnvelope, CloudEventsCoordinator, CloudEventsSink, ContentType, HttpSink,
    HttpSinkConfig, MultiSink, NullSink, SinkError, SinkType,
};
#[cfg(feature = "kafka")]
pub use cloudevents::{KafkaSink, KafkaSinkConfig};
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub use event::{connect_pool, EventService, EventServiceHandle};
pub use log::{LogService, LogServiceHandle};
pub use outbound::{OutboundEventHandler, OutboundService};
pub use stream::StreamService;
