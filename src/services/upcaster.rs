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
use tonic::{Request, Status};
use tracing::{debug, info};

use crate::proto::{upcaster_client::UpcasterClient, EventPage, UpcastRequest};

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
            enabled: std::env::var("ANGZARR_UPCASTER_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            address: std::env::var("ANGZARR_UPCASTER_ADDRESS")
                .unwrap_or_else(|_| "localhost:50053".to_string()),
            timeout_ms: 5000,
        }
    }
}

impl UpcasterConfig {
    /// Check if upcaster is enabled (config or env var).
    pub fn is_enabled(&self) -> bool {
        self.enabled
            || std::env::var("ANGZARR_UPCASTER_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false)
    }

    /// Get the upcaster address (config or env var).
    pub fn get_address(&self) -> String {
        std::env::var("ANGZARR_UPCASTER_ADDRESS").unwrap_or_else(|_| self.address.clone())
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
    client: Option<Arc<Mutex<UpcasterClient<Channel>>>>,
    #[allow(dead_code)] // Reserved for reconnection logic
    config: UpcasterConfig,
}

impl Upcaster {
    /// Create a new upcaster client.
    ///
    /// If upcaster is disabled, creates a passthrough client that returns events unchanged.
    pub async fn new(config: UpcasterConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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

        let client = UpcasterClient::new(channel);
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

        let request = Request::new(UpcastRequest {
            domain: domain.to_string(),
            events,
        });

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
}
