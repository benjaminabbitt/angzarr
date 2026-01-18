//! Projector implementations.
//!
//! Contains actual projector services that process events and produce output.
//! These implement the ProjectorCoordinator gRPC service.

pub mod log;
pub mod stream;

pub use log::{LogService, LogServiceHandle};
pub use stream::{StreamEventHandler, StreamService};
