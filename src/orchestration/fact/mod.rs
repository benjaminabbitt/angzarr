//! Fact injection abstraction.
//!
//! `FactExecutor` injects facts (external events) into target aggregates.
//! - `local/`: calls in-process fact pipeline directly
//! - `grpc/`: calls remote `CommandHandlerCoordinatorServiceClient::handle_event` via gRPC

pub mod grpc;
pub mod local;
