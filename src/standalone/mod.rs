//! Standalone runtime for running angzarr as a single process.
//!
//! This module provides a simplified API for running all angzarr components
//! in a single process with user-registered handlers.
//!
//! # Example
//!
//! ```ignore
//! use angzarr::standalone::{Runtime, AggregateHandler, ProjectorHandler};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let runtime = Runtime::builder()
//!         .with_sqlite_memory()
//!         .register_aggregate("orders", MyOrdersHandler)
//!         .register_projector("accounting", MyAccountingProjector, Default::default())
//!         .build()
//!         .await?;
//!
//!     runtime.run().await
//! }
//! ```

mod builder;
mod client;
pub mod edition;
pub mod grpc_handlers;
mod router;
mod runtime;
mod server;
mod speculative;
mod traits;

pub use builder::{GatewayConfig, RuntimeBuilder};
pub use client::{CommandBuilder, CommandClient, EditionClient, SpeculativeClient, StandaloneQueryClient};
pub use edition::{
    EditionAggregateContext, EditionEventStore, EditionManager, EditionMetadata, DivergencePoint,
};
pub use grpc_handlers::{AggregateHandlerAdapter, GrpcProjectorHandler};
pub use router::{CommandRouter, DomainStorage, SyncProjectorEntry};
pub use runtime::Runtime;
pub use server::{ServerInfo, StandaloneEventQueryBridge, StandaloneGatewayService};
pub use speculative::{DomainStateSpec, PmSpeculativeResult, SpeculativeExecutor};
pub use traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectionMode, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};
