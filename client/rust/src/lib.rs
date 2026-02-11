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
//!     let response = client.aggregate
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

pub mod builder;
pub mod client;
pub mod convert;
pub mod error;
pub mod proto;
pub mod proto_ext;
pub mod traits;

// Re-export main types at crate root
pub use client::{AggregateClient, Client, DomainClient, QueryClient, SpeculativeClient};
pub use error::{ClientError, Result};

// Re-export builder extension traits for fluent API
pub use builder::{CommandBuilderExt, QueryBuilderExt};

// Re-export helpers
pub use builder::{decode_event, events_from_response, root_from_cover};
pub use convert::{
    now, parse_timestamp, proto_to_uuid, type_name_from_url, type_url, type_url_matches,
    uuid_to_proto, TYPE_URL_PREFIX,
};

// Re-export extension traits
pub use proto_ext::{
    CommandBookExt, CommandPageExt, CoverExt, EditionExt, EventBookExt, EventPageExt, ProtoUuidExt,
    UuidExt,
};
