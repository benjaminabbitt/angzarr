//! Upcaster client for event version transformation.
//!
//! Transforms old event versions to current versions by calling an external
//! upcaster service. Deployed as an optional sidecar container in the aggregate pod.
//!
//! # Configuration
//!
//! ```yaml
//! upcaster:
//!   enabled: true
//!   address: "localhost:50053"
//!   timeout_ms: 5000
//! ```
//!
//! Or via environment:
//! - `ANGZARR_UPCASTER_ENABLED=true`
//! - `ANGZARR_UPCASTER_ADDRESS=localhost:50053`

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
    /// Upcaster service address. Default: "localhost:50053".
    /// Can be overridden via ANGZARR_UPCASTER_ADDRESS env var.
    pub address: String,
    /// Timeout in milliseconds. Default: 5000.
    pub timeout_ms: u64,
}

impl Default for UpcasterConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var(UPCASTER_ENABLED_ENV_VAR)
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            address: std::env::var(UPCASTER_ADDRESS_ENV_VAR)
                .unwrap_or_else(|_| "localhost:50053".to_string()),
            timeout_ms: 5000,
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

    /// Get the upcaster address (config or env var).
    pub fn get_address(&self) -> String {
        std::env::var(UPCASTER_ADDRESS_ENV_VAR).unwrap_or_else(|_| self.address.clone())
    }
}

// ============================================================================
// Client
// ============================================================================

/// Upcaster client wrapper.
///
/// Handles connection to the upcaster service and provides a simple interface
/// for transforming events.
pub struct Upcaster {
    client: Option<Arc<Mutex<UpcasterServiceClient<Channel>>>>,
    #[allow(dead_code)] // Reserved for reconnection logic
    config: UpcasterConfig,
}

impl Upcaster {
    /// Create a new upcaster client.
    ///
    /// If upcaster is disabled, creates a passthrough client that returns events unchanged.
    pub async fn new(
        config: UpcasterConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !config.is_enabled() {
            info!("Upcaster disabled");
            return Ok(Self {
                client: None,
                config,
            });
        }

        let address = config.get_address();
        let endpoint = format!("http://{}", address);

        let channel = Channel::from_shared(endpoint.clone())?
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .connect()
            .await?;

        let client = UpcasterServiceClient::new(channel);
        info!(address = %address, "Upcaster client connected");

        Ok(Self {
            client: Some(Arc::new(Mutex::new(client))),
            config,
        })
    }

    /// Create a disabled upcaster (passthrough).
    pub fn disabled() -> Self {
        Self {
            client: None,
            config: UpcasterConfig::default(),
        }
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
        assert_eq!(config.address, "localhost:50053");
        assert_eq!(config.timeout_ms, 5000);
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
    use crate::proto::upcaster_service_server::{
        Upcaster as UpcasterService, UpcasterServiceServer,
    };
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

        let config = UpcasterConfig {
            enabled: true,
            address: addr.to_string(),
            timeout_ms: 5000,
        };

        let upcaster = Upcaster::new(config).await.expect("Failed to connect");
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

        let config = UpcasterConfig {
            enabled: true,
            address: addr.to_string(),
            timeout_ms: 5000,
        };

        let upcaster = Upcaster::new(config).await.unwrap();

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

        let config = UpcasterConfig {
            enabled: true,
            address: addr.to_string(),
            timeout_ms: 5000,
        };

        let upcaster = Upcaster::new(config).await.unwrap();

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

        let config = UpcasterConfig {
            enabled: true,
            address: addr.to_string(),
            timeout_ms: 5000,
        };

        let upcaster = Upcaster::new(config).await.unwrap();

        // Empty events should short-circuit without calling server
        let result = upcaster.upcast("test", vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_upcaster_error_propagation() {
        let addr = start_mock_server(MockUpcasterService::failing()).await;

        let config = UpcasterConfig {
            enabled: true,
            address: addr.to_string(),
            timeout_ms: 5000,
        };

        let upcaster = Upcaster::new(config).await.unwrap();

        let events = vec![make_test_event(0, "example.Event", vec![1])];

        let result = upcaster.upcast("test", events).await;

        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert!(status.message().contains("Simulated upcaster failure"));
    }

    #[tokio::test]
    async fn test_upcaster_connection_failure() {
        let config = UpcasterConfig {
            enabled: true,
            address: "127.0.0.1:1".to_string(), // Invalid port
            timeout_ms: 100,
        };

        let result = Upcaster::new(config).await;
        assert!(result.is_err());
    }
}
