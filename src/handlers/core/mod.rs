//! Core angzarr sidecar handlers.
//!
//! These handlers receive events from the AMQP bus and forward them to
//! business logic coordinators (projectors, sagas) via gRPC.

pub mod projector;
pub mod saga;

pub use projector::ProjectorEventHandler;
pub use saga::SagaEventHandler;
