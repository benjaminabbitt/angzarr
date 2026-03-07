//! Ergonomic Rust client for Angzarr gRPC services.
//!
//! This crate provides typed clients with fluent builder APIs for interacting
//! with Angzarr aggregate coordinator and query services.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use angzarr_client::{DomainClient, CommandBuilderExt, QueryBuilderExt};
//! use uuid::Uuid;
//!
//! async fn example() -> angzarr_client::Result<()> {
//!     // Connect to a domain's coordinator
//!     let client = DomainClient::connect("http://localhost:1310").await?;
//!
//!     // Execute a command
//!     let cart_id = Uuid::new_v4();
//!     let response = client.command_handler
//!         .command("cart", cart_id)
//!         .with_command("type.googleapis.com/examples.CreateCart", &create_cart)
//!         .execute()
//!         .await?;
//!
//!     // Query events
//!     let events = client.query
//!         .query("cart", cart_id)
//!         .range(0)
//!         .get_pages()
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! # Mocking for Tests
//!
//! Implement the `GatewayClient` and `QueryClient` traits to create mock clients:
//!
//! ```rust,ignore
//! use angzarr_client::traits::{GatewayClient, QueryClient};
//! use angzarr_client::proto::CommandBook;
//! use async_trait::async_trait;
//!
//! struct MockAggregate;
//!
//! #[async_trait]
//! impl GatewayClient for MockAggregate {
//!     async fn execute(&self, _cmd: CommandBook)
//!         -> angzarr_client::Result<angzarr_client::proto::CommandResponse>
//!     {
//!         // Return mock response
//!         Ok(angzarr_client::proto::CommandResponse::default())
//!     }
//! }
//! ```

/// Version of the angzarr-client crate, injected at build time from VERSION file.
pub const VERSION: &str = env!("ANGZARR_CLIENT_VERSION");

pub mod builder;
pub mod client;
pub mod convert;
pub mod error;
pub mod handler;
pub mod proto;
pub mod proto_ext;
pub mod router;
pub mod server;
pub mod traits;
pub mod validation;

// Re-export main types at crate root
pub use client::{CommandHandlerClient, DomainClient, QueryClient, SpeculativeClient};
pub use error::{ClientError, Result};

// Re-export builder extension traits for fluent API
pub use builder::{CommandBuilderExt, QueryBuilderExt};

// Re-export helpers
pub use builder::{decode_event, events_from_response, root_from_cover};
pub use convert::{
    full_type_name, full_type_url, now, parse_timestamp, proto_to_uuid, try_unpack, type_matches,
    type_name_from_url, type_url, type_url_matches_exact, unpack, uuid_to_proto, TYPE_URL_PREFIX,
};

// Re-export extension traits
pub use proto_ext::{
    CommandBookExt, CommandPageExt, CoverExt, EditionExt, EventBookExt, EventPageExt, ProtoUuidExt,
    UuidExt,
};

// Re-export router types
pub use router::{
    // Helper functions
    event_book_from,
    event_page,
    new_event_book,
    new_event_book_multi,
    pack_event,
    // Upcaster types
    BoxedUpcasterHandler,
    // CloudEvents types
    CloudEventsHandler,
    CloudEventsProjector,
    CloudEventsRouter,
    // Handler traits
    CommandHandlerDomainHandler,
    // Mode markers
    CommandHandlerMode,
    // Router types
    CommandHandlerRouter,
    // Error types
    CommandRejectedError,
    CommandResult,
    // State management
    EventApplier,
    ProcessManagerDomainHandler,
    ProcessManagerMode,
    ProcessManagerResponse,
    ProcessManagerRouter,
    ProjectorDomainHandler,
    ProjectorMode,
    ProjectorRouter,
    RejectionHandlerResponse,
    // Saga types
    SagaContext,
    SagaDomainHandler,
    SagaHandlerResponse,
    SagaMode,
    SagaRouter,
    StateFactory,
    StateRouter,
    UnpackAny,
    UpcasterHandler,
    UpcasterMode,
    UpcasterRouter,
};

// Note: dispatch_command! and dispatch_event! macros are available at crate root
// via #[macro_export] in router/dispatch.rs

// Re-export handler types
pub use handler::{
    CloudEventsGrpcHandler, CommandHandlerGrpc, ProcessManagerGrpcHandler, ProjectorHandler,
    SagaHandler, StatePacker, UpcasterGrpcHandler, UpcasterHandleClosureFn, UpcasterHandleFn,
};

// Re-export server utilities
pub use server::{
    run_cloudevents_projector, run_command_handler_server, run_process_manager_server,
    run_projector_server, run_saga_server, run_upcaster_server, ServerConfig,
};

// Re-export validation helpers
pub use validation::{
    require_exists, require_non_negative, require_not_empty, require_not_empty_str,
    require_not_exists, require_positive, require_status, require_status_not,
};
