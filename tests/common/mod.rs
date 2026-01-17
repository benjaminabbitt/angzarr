//! Shared utilities for integration tests.
//!
//! Provides gRPC client helpers for testing against a deployed Kind cluster.

use prost::Message;
use tonic::transport::Channel;
use uuid::Uuid;

pub use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_query_client::EventQueryClient,
    CommandBook, CommandPage, CommandResponse, Cover, EventBook, ExecuteStreamCountRequest, Query,
    Uuid as ProtoUuid,
};

/// Default gateway endpoint for Kind cluster
pub const DEFAULT_GATEWAY_ENDPOINT: &str = "http://localhost:50051";

/// Default query endpoint (proxied through gateway)
pub const DEFAULT_QUERY_ENDPOINT: &str = "http://localhost:50051";

/// Creates a CommandGatewayClient connected to the gateway.
pub async fn create_gateway_client() -> CommandGatewayClient<Channel> {
    let endpoint = std::env::var("ANGZARR_GATEWAY_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_GATEWAY_ENDPOINT.to_string());

    let channel = Channel::from_shared(endpoint)
        .expect("Invalid gateway endpoint")
        .connect()
        .await
        .expect("Failed to connect to gateway");

    CommandGatewayClient::new(channel)
}

/// Creates an EventQueryClient connected to the query service.
pub async fn create_query_client() -> EventQueryClient<Channel> {
    let endpoint = std::env::var("ANGZARR_QUERY_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_QUERY_ENDPOINT.to_string());

    let channel = Channel::from_shared(endpoint)
        .expect("Invalid query endpoint")
        .connect()
        .await
        .expect("Failed to connect to query service");

    EventQueryClient::new(channel)
}

/// Builds a CommandBook for sending commands to the gateway.
pub fn build_command_book(
    domain: &str,
    root: Uuid,
    command: impl Message,
    type_url: &str,
) -> CommandBook {
    let correlation_id = Uuid::new_v4().to_string();
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
        }),
        pages: vec![CommandPage {
            sequence: 0,
            synchronous: false,
            command: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", type_url),
                value: command.encode_to_vec(),
            }),
        }],
        correlation_id,
        saga_origin: None,
        auto_resequence: false,
        fact: false,
    }
}

/// Builds a Query for retrieving events from an aggregate.
pub fn build_query(domain: &str, root: Uuid) -> Query {
    Query {
        domain: domain.to_string(),
        root: Some(ProtoUuid {
            value: root.as_bytes().to_vec(),
        }),
        lower_bound: 0,
        upper_bound: u32::MAX,
    }
}

/// Extracts the event type name from a protobuf Any.
pub fn extract_event_type(event: &prost_types::Any) -> String {
    event
        .type_url
        .rsplit('/')
        .next()
        .unwrap_or(&event.type_url)
        .to_string()
}

/// Checks if integration tests should run.
/// Returns true if ANGZARR_TEST_MODE=container.
pub fn should_run_integration_tests() -> bool {
    std::env::var("ANGZARR_TEST_MODE")
        .map(|v| v.to_lowercase() == "container")
        .unwrap_or(false)
}

/// Macro to skip test if not in container mode.
#[macro_export]
macro_rules! skip_if_not_container {
    () => {
        if !$crate::integration::should_run_integration_tests() {
            eprintln!("Skipping: Set ANGZARR_TEST_MODE=container to run");
            return;
        }
    };
}
