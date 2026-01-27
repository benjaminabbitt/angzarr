//! Logging projector gRPC server.
//!
//! Receives events and logs them with structured tracing.
//! Domain can be specified via DOMAIN environment variable.

use projector_logging::LoggingProjector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "all".to_string());
    common::run_projector_server("logging", &domain, "50163", LoggingProjector::new(&domain)).await
}
