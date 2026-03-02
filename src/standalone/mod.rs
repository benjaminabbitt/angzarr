//! Standalone runtime for running angzarr as a single process.
//!
//! This module provides a simplified API for running all angzarr components
//! in a single process with user-registered handlers.
//!
//! # Example
//!
//! ```ignore
//! use angzarr::standalone::{Runtime, CommandHandler, ProjectorHandler};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let runtime = Runtime::builder()
//!         .with_sqlite_memory()
//!         .register_command_handler("orders", MyOrdersHandler)
//!         .register_projector("accounting", MyAccountingProjector, Default::default())
//!         .build()
//!         .await?;
//!
//!     runtime.run().await
//! }
//! ```

/// Error message constants for standalone mode.
pub mod errmsg {
    // Re-export common service error messages
    pub use crate::services::errmsg::{
        COMMAND_REQUEST_MISSING_COMMAND, EVENT_REQUEST_MISSING_EVENTS, QUERY_MISSING_COVER,
        QUERY_MISSING_ROOT, SPECULATE_AGG_MISSING_COMMAND,
    };

    /// Missing command in request.
    pub const MISSING_COMMAND: &str = "Missing command";
    /// Empty command pages.
    pub const EMPTY_COMMAND_PAGES: &str = "Empty command pages";
    /// Missing command payload.
    pub const MISSING_COMMAND_PAYLOAD: &str = "Missing command payload";
    /// SpeculateCommandHandlerRequest missing command.
    pub const SPECULATE_CMD_MISSING_COMMAND: &str =
        "SpeculateCommandHandlerRequest must have a command";
}

mod builder;
mod client;
mod dispatcher;
pub mod grpc_handlers;
mod router;
mod runtime;
mod server;
mod speculative;
mod traits;

pub use builder::RuntimeBuilder;
pub use client::{CommandBuilder, CommandClient, SpeculativeClient, StandaloneQueryClient};
pub use dispatcher::CommandDispatcher;
pub use grpc_handlers::{CommandHandlerAdapter, GrpcProjectorHandler};
pub use router::{CommandRouter, DomainStorage, SyncProjectorEntry, SyncSagaEntry};
pub use runtime::Runtime;
pub use server::{ServerInfo, SingleDomainEventQuery, StandaloneAggregateService};
pub use speculative::{DomainStateSpec, PmSpeculativeResult, SpeculativeExecutor};
pub use traits::{
    CommandHandler, ProcessManagerConfig, ProcessManagerHandleResult, ProcessManagerHandler,
    ProjectionMode, ProjectorConfig, ProjectorHandler, SagaConfig, SagaHandler,
};
