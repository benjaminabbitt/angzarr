//! Projector implementations.
//!
//! Contains actual projector services that process events and produce output.
//! These implement the ProjectorCoordinator gRPC service.

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod event;
pub mod log;
pub mod stream;
#[cfg(feature = "topology")]
pub mod topology;

#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub use event::{connect_pool, EventService, EventServiceHandle};
pub use log::{LogService, LogServiceHandle};
pub use stream::StreamService;
