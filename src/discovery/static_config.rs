//! Static service configuration for local development.
//!
//! Parses environment variables to configure service endpoints when
//! Kubernetes API is not available.

use std::sync::Arc;

use tracing::{info, warn};

use super::registry::{ServiceEndpoint, ServiceRegistry};

/// Parse service configuration from environment variables.
///
/// Supports two formats:
/// 1. `COMMAND_HANDLERS=domain1=addr1:port1,domain2=addr2:port2`
/// 2. `COMMAND_ADDRESS=addr:port` (legacy, registers as wildcard "*")
pub async fn load_from_env(registry: Arc<ServiceRegistry>) -> Result<(), String> {
    // Try COMMAND_HANDLERS first (multi-domain)
    if let Ok(handlers) = std::env::var("COMMAND_HANDLERS") {
        info!("Loading command handlers from COMMAND_HANDLERS env var");
        parse_command_handlers(&handlers, registry).await?;
        return Ok(());
    }

    // Fall back to COMMAND_ADDRESS (single/wildcard)
    if let Ok(addr) = std::env::var("COMMAND_ADDRESS") {
        info!(
            address = %addr,
            "Loading single command handler from COMMAND_ADDRESS (wildcard mode)"
        );
        let (address, port) = parse_address(&addr)?;
        registry
            .update_endpoint(ServiceEndpoint {
                domain: "*".to_string(),
                address,
                port,
            })
            .await;
        return Ok(());
    }

    Err("No service configuration found. Set COMMAND_HANDLERS or COMMAND_ADDRESS".to_string())
}

/// Parse COMMAND_HANDLERS format: "domain1=addr1:port1,domain2=addr2:port2"
async fn parse_command_handlers(handlers: &str, registry: Arc<ServiceRegistry>) -> Result<(), String> {
    let mut count = 0;

    for pair in handlers.split(',') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() != 2 {
            warn!(entry = %pair, "Invalid COMMAND_HANDLERS entry, skipping");
            continue;
        }

        let domain = parts[0].trim().to_string();
        let addr_str = parts[1].trim();

        let (address, port) = parse_address(addr_str).map_err(|e| {
            format!("Invalid address for domain '{}': {}", domain, e)
        })?;

        info!(
            domain = %domain,
            address = %address,
            port = port,
            "Registered command handler"
        );

        registry
            .update_endpoint(ServiceEndpoint {
                domain,
                address,
                port,
            })
            .await;

        count += 1;
    }

    if count == 0 {
        return Err("COMMAND_HANDLERS is empty or has no valid entries".to_string());
    }

    info!(count = count, "Loaded command handlers from environment");
    Ok(())
}

/// Parse address string "host:port" or "host" (default port 50051).
fn parse_address(addr: &str) -> Result<(String, u16), String> {
    let parts: Vec<&str> = addr.trim().rsplitn(2, ':').collect();

    match parts.len() {
        1 => {
            // No port specified, use default
            Ok((parts[0].to_string(), 50051))
        }
        2 => {
            let port: u16 = parts[0]
                .parse()
                .map_err(|_| format!("Invalid port number: {}", parts[0]))?;
            Ok((parts[1].to_string(), port))
        }
        _ => Err(format!("Invalid address format: {}", addr)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_address_with_port() {
        let (host, port) = parse_address("localhost:8080").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_address_without_port() {
        let (host, port) = parse_address("localhost").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 50051); // default
    }

    #[test]
    fn test_parse_address_with_hostname_and_port() {
        let (host, port) = parse_address("angzarr-customer.angzarr.svc.cluster.local:50051").unwrap();
        assert_eq!(host, "angzarr-customer.angzarr.svc.cluster.local");
        assert_eq!(port, 50051);
    }

    #[test]
    fn test_parse_address_invalid_port() {
        let result = parse_address("localhost:invalid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_command_handlers() {
        let registry = Arc::new(ServiceRegistry::new());
        let handlers = "customer=localhost:50051,transaction=localhost:50052";

        parse_command_handlers(handlers, registry.clone()).await.unwrap();

        assert!(registry.has_domain("customer").await);
        assert!(registry.has_domain("transaction").await);
        assert_eq!(registry.len().await, 2);
    }

    #[tokio::test]
    async fn test_parse_command_handlers_with_spaces() {
        let registry = Arc::new(ServiceRegistry::new());
        let handlers = " customer = localhost:50051 , transaction = localhost:50052 ";

        parse_command_handlers(handlers, registry.clone()).await.unwrap();

        assert!(registry.has_domain("customer").await);
        assert!(registry.has_domain("transaction").await);
    }

    #[tokio::test]
    async fn test_parse_command_handlers_empty() {
        let registry = Arc::new(ServiceRegistry::new());
        let result = parse_command_handlers("", registry).await;
        assert!(result.is_err());
    }
}
