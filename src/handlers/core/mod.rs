//! Core angzarr sidecar handlers.
//!
//! These handlers receive events/commands from buses and forward them to
//! client logic coordinators (aggregates, projectors, sagas, process managers) via gRPC.

pub mod aggregate;
pub mod process_manager;
pub mod projector;
pub mod saga;

pub use aggregate::{wrap_command_for_bus, AggregateCommandHandler, SyncProjectorEntry};
pub use process_manager::ProcessManagerEventHandler;
pub use projector::ProjectorEventHandler;
pub use saga::SagaEventHandler;
