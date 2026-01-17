//! Server and networking configuration types.

use serde::Deserialize;

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Port for entity gRPC service.
    pub entity_port: u16,
    /// Port for event query gRPC service.
    pub event_query_port: u16,
    /// Host to bind to.
    pub host: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            entity_port: 1313,
            event_query_port: 1314,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// Target service configuration for sidecar modes.
#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    /// gRPC address of the target service.
    pub address: String,
    /// Domain handled by this service (for aggregate mode).
    pub domain: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let server = ServerConfig::default();
        assert_eq!(server.entity_port, 1313);
        assert_eq!(server.event_query_port, 1314);
        assert_eq!(server.host, "0.0.0.0");
    }
}
