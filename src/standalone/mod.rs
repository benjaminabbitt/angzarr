//! Embedded runtime for running angzarr as a single process.
//!
//! This module provides a simplified API for running all angzarr components
//! in a single process with user-registered handlers.
//!
//! # Example
//!
//! ```ignore
//! use angzarr::embedded::{Runtime, AggregateHandler, ProjectorHandler};
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
mod router;
mod runtime;
mod server;
mod traits;

pub use builder::{GatewayConfig, RuntimeBuilder};
pub use client::{CommandBuilder, CommandClient, StandaloneQueryClient};
pub use router::{CommandRouter, DomainStorage};
pub use runtime::Runtime;
pub use traits::{
    AggregateHandler, ProcessManagerConfig, ProcessManagerHandler, ProjectorConfig,
    ProjectorHandler, SagaConfig, SagaHandler,
};
