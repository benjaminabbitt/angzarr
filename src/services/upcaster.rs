//! Upcaster client for event version transformation.
//!
//! Transforms old event versions to current versions by calling the client's
//! UpcasterService implementation. The client binary implements both
//! AggregateService and UpcasterService on the same gRPC server.
//!
//! # Configuration
//!
//! ```yaml
//! upcaster:
//!   enabled: true
//!   # Optional: override address (defaults to client logic address)
//!   address: "localhost:50053"
//! ```
//!
//! Or via environment:
//! - `ANGZARR_UPCASTER_ENABLED=true`
//! - `ANGZARR_UPCASTER_ADDRESS=localhost:50053` (optional override)

use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use tonic::Status;
use tracing::{debug, info};

use crate::config::{UPCASTER_ADDRESS_ENV_VAR, UPCASTER_ENABLED_ENV_VAR};
use crate::proto::{upcaster_service_client::UpcasterServiceClient, EventPage, UpcastRequest};
use crate::proto_ext::correlated_request;

// ============================================================================
// Configuration
// ============================================================================

/// Upcaster configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UpcasterConfig {
    /// Enable upcaster. Default: false.
    /// Can be overridden via ANGZARR_UPCASTER_ENABLED env var.
    pub enabled: bool,
    /// Optional address override. If not set, uses the same address as client logic.
    /// Can be overridden via ANGZARR_UPCASTER_ADDRESS env var.
    #[serde(default)]
    pub address: Option<String>,
}

impl Default for UpcasterConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var(UPCASTER_ENABLED_ENV_VAR)
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            address: std::env::var(UPCASTER_ADDRESS_ENV_VAR).ok(),
        }
    }
}

impl UpcasterConfig {
    /// Check if upcaster is enabled (config or env var).
    pub fn is_enabled(&self) -> bool {
        self.enabled
            || std::env::var(UPCASTER_ENABLED_ENV_VAR)
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false)
    }

    /// Get optional address override (config or env var).
    pub fn get_address_override(&self) -> Option<String> {
        std::env::var(UPCASTER_ADDRESS_ENV_VAR)
            .ok()
            .or_else(|| self.address.clone())
    }
}

// ============================================================================
// Client
// ============================================================================

/// Upcaster client wrapper.
///
/// Calls the client's UpcasterService to transform old event versions.
/// By default uses the same gRPC channel as client logic (AggregateService).
pub struct Upcaster {
    client: Option<Arc<Mutex<UpcasterServiceClient<Channel>>>>,
}

impl Upcaster {
    /// Create an upcaster client from an existing channel.
    ///
    /// Uses the same channel as client logic (both services on same server).
    pub fn from_channel(channel: Channel) -> Self {
        let client = UpcasterServiceClient::new(channel);
        info!("Upcaster client created (shared channel with client logic)");
        Self {
            client: Some(Arc::new(Mutex::new(client))),
        }
    }

    /// Create an upcaster client with a separate address.
    ///
    /// Used when upcaster runs as a separate sidecar.
    pub async fn from_address(address: &str) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::transport::connect_to_address;

        let channel = connect_to_address(address).await?;
        let client = UpcasterServiceClient::new(channel);
        info!(address = %address, "Upcaster client connected (separate address)");

        Ok(Self {
            client: Some(Arc::new(Mutex::new(client))),
        })
    }

    /// Create a disabled upcaster (passthrough).
    pub fn disabled() -> Self {
        Self { client: None }
    }

    /// Check if upcaster is enabled.
    pub fn is_enabled(&self) -> bool {
        self.client.is_some()
    }

    /// Transform events to current version.
    ///
    /// If upcaster is disabled, returns events unchanged.
    pub async fn upcast(
        &self,
        domain: &str,
        events: Vec<EventPage>,
    ) -> Result<Vec<EventPage>, Status> {
        let client = match &self.client {
            Some(c) => c,
            None => return Ok(events), // Passthrough when disabled
        };

        if events.is_empty() {
            return Ok(events);
        }

        debug!(domain = %domain, event_count = events.len(), "Upcasting events");

        let request = correlated_request(
            UpcastRequest {
                domain: domain.to_string(),
                events,
            },
            "", // No correlation context for upcasting
        );

        let mut client = client.lock().await;
        let response = client.upcast(request).await?;

        Ok(response.into_inner().events)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upcaster_config_default() {
        let config = UpcasterConfig::default();
        assert!(!config.enabled);
        assert!(config.address.is_none());
    }

    #[test]
    fn test_upcaster_disabled() {
        let upcaster = Upcaster::disabled();
        assert!(!upcaster.is_enabled());
    }

    #[tokio::test]
    async fn test_upcaster_passthrough_when_disabled() {
        let upcaster = Upcaster::disabled();

        let events = vec![EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(1)),
            created_at: None,
            event: None,
        }];

        let result = upcaster.upcast("test", events.clone()).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_upcaster_passthrough_empty_events() {
        let upcaster = Upcaster::disabled();
        let result = upcaster.upcast("test", vec![]).await.unwrap();
        assert!(result.is_empty());
    }
}

#[cfg(test)]
mod grpc_tests {
    use super::*;
    use crate::proto::event_page::Sequence;
    use crate::proto::upcaster_service_server::{UpcasterService, UpcasterServiceServer};
    use crate::proto::{UpcastRequest, UpcastResponse};
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tonic::transport::Server;
    use tonic::{Request, Response};

    /// Mock upcaster that transforms event type_urls from V1 to V2.
    struct MockUpcasterService {
        call_count: AtomicU32,
        should_fail: bool,
    }

    impl MockUpcasterService {
        fn new() -> Self {
            Self {
                call_count: AtomicU32::new(0),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                call_count: AtomicU32::new(0),
                should_fail: true,
            }
        }
    }

    #[tonic::async_trait]
    impl UpcasterService for MockUpcasterService {
        async fn upcast(
            &self,
            request: Request<UpcastRequest>,
        ) -> Result<Response<UpcastResponse>, tonic::Status> {
            self.call_count.fetch_add(1, Ordering::SeqCst);

            if self.should_fail {
                return Err(tonic::Status::internal("Simulated upcaster failure"));
            }

            let req = request.into_inner();

            // Transform events: rename V1 type_urls to V2
            let transformed: Vec<EventPage> = req
                .events
                .into_iter()
                .map(|mut page| {
                    if let Some(ref mut event) = page.event {
                        // Simulate V1 -> V2 transformation
                        if event.type_url.ends_with("V1") {
                            event.type_url = event.type_url.replace("V1", "V2");
                        }
                        // Simulate field migration: add marker byte
                        if !event.value.is_empty() {
                            event.value.push(0xFF); // Migration marker
                        }
                    }
                    page
                })
                .collect();

            Ok(Response::new(UpcastResponse {
                events: transformed,
            }))
        }
    }

    /// Start a mock upcaster server and return its address.
    async fn start_mock_server(service: MockUpcasterService) -> SocketAddr {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            Server::builder()
                .add_service(UpcasterServiceServer::new(service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });

        // Give server time to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        local_addr
    }

    fn make_test_event(seq: u32, type_url: &str, value: Vec<u8>) -> EventPage {
        EventPage {
            sequence: Some(Sequence::Num(seq)),
            created_at: None,
            event: Some(prost_types::Any {
                type_url: type_url.to_string(),
                value,
            }),
        }
    }

    #[tokio::test]
    async fn test_upcaster_transforms_events() {
        let addr = start_mock_server(MockUpcasterService::new()).await;

        let upcaster = Upcaster::from_address(&addr.to_string())
            .await
            .expect("Failed to connect");
        assert!(upcaster.is_enabled());

        let events = vec![
            make_test_event(0, "example.OrderCreatedV1", vec![1, 2, 3]),
            make_test_event(1, "example.OrderUpdatedV1", vec![4, 5, 6]),
        ];

        let result = upcaster.upcast("order", events).await.unwrap();

        assert_eq!(result.len(), 2);

        // Verify V1 -> V2 transformation
        let event0 = result[0].event.as_ref().unwrap();
        assert_eq!(event0.type_url, "example.OrderCreatedV2");
        assert_eq!(event0.value, vec![1, 2, 3, 0xFF]); // Migration marker added

        let event1 = result[1].event.as_ref().unwrap();
        assert_eq!(event1.type_url, "example.OrderUpdatedV2");
        assert_eq!(event1.value, vec![4, 5, 6, 0xFF]);
    }

    #[tokio::test]
    async fn test_upcaster_preserves_non_v1_events() {
        let addr = start_mock_server(MockUpcasterService::new()).await;

        let upcaster = Upcaster::from_address(&addr.to_string()).await.unwrap();

        let events = vec![
            make_test_event(0, "example.OrderCreated", vec![1, 2]), // No V1 suffix
        ];

        let result = upcaster.upcast("order", events).await.unwrap();

        assert_eq!(result.len(), 1);
        let event = result[0].event.as_ref().unwrap();
        assert_eq!(event.type_url, "example.OrderCreated"); // Unchanged
        assert_eq!(event.value, vec![1, 2, 0xFF]); // But value still gets marker
    }

    #[tokio::test]
    async fn test_upcaster_preserves_sequence_numbers() {
        let addr = start_mock_server(MockUpcasterService::new()).await;

        let upcaster = Upcaster::from_address(&addr.to_string()).await.unwrap();

        let events = vec![
            make_test_event(5, "example.EventV1", vec![]),
            make_test_event(6, "example.EventV1", vec![]),
            make_test_event(7, "example.EventV1", vec![]),
        ];

        let result = upcaster.upcast("test", events).await.unwrap();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].sequence, Some(Sequence::Num(5)));
        assert_eq!(result[1].sequence, Some(Sequence::Num(6)));
        assert_eq!(result[2].sequence, Some(Sequence::Num(7)));
    }

    #[tokio::test]
    async fn test_upcaster_handles_empty_events() {
        let addr = start_mock_server(MockUpcasterService::new()).await;

        let upcaster = Upcaster::from_address(&addr.to_string()).await.unwrap();

        // Empty events should short-circuit without calling server
        let result = upcaster.upcast("test", vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_upcaster_error_propagation() {
        let addr = start_mock_server(MockUpcasterService::failing()).await;

        let upcaster = Upcaster::from_address(&addr.to_string()).await.unwrap();

        let events = vec![make_test_event(0, "example.Event", vec![1])];

        let result = upcaster.upcast("test", events).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(status.message().contains("Simulated upcaster failure"));
    }

    #[tokio::test]
    async fn test_upcaster_connection_failure() {
        let result = Upcaster::from_address("127.0.0.1:1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_upcaster_from_channel() {
        let addr = start_mock_server(MockUpcasterService::new()).await;

        // Connect via channel (simulates sharing channel with client logic)
        let channel = Channel::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .unwrap();

        let upcaster = Upcaster::from_channel(channel);
        assert!(upcaster.is_enabled());

        let events = vec![make_test_event(0, "example.EventV1", vec![1])];
        let result = upcaster.upcast("test", events).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].event.as_ref().unwrap().type_url,
            "example.EventV2"
        );
    }
}
