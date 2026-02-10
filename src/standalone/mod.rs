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
pub mod grpc_handlers;
mod meta_aggregate;
mod router;
mod runtime;
mod server;
mod speculative;
mod traits;

pub use builder::RuntimeBuilder;
pub use client::{CommandBuilder, CommandClient, SpeculativeClient, StandaloneQueryClient};
pub use grpc_handlers::{AggregateHandlerAdapter, GrpcProjectorHandler};
pub use meta_aggregate::MetaAggregateHandler;
// Re-export meta domain constants from proto_ext for consistency
pub use crate::proto_ext::{
    component_name_to_uuid, COMPONENT_REGISTERED_TYPE_URL, META_ANGZARR_DOMAIN as META_DOMAIN,
    REGISTER_COMPONENT_TYPE_URL,
};
pub use router::{CommandRouter, DomainStorage, SyncProjectorEntry};
pub use runtime::Runtime;
pub use server::{ServerInfo, SingleDomainEventQuery, StandaloneAggregateService};
pub use speculative::{DomainStateSpec, PmSpeculativeResult, SpeculativeExecutor};
pub use traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectionMode, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};
