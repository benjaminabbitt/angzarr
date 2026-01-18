//! Bootstrap utilities for angzarr binaries.
//!
//! Shared initialization code for all angzarr sidecar binaries.

use std::future::Future;
use std::time::Duration;

use tracing::warn;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize tracing with ANGZARR_LOG environment variable.
///
/// Defaults to "info" level if ANGZARR_LOG is not set.
pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_env("ANGZARR_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Connect to a gRPC service with exponential backoff retry.
///
/// # Arguments
/// * `service_name` - Human-readable name for logging (e.g., "projector", "saga")
/// * `address` - The gRPC address to connect to
/// * `connect` - Async function that attempts to establish a connection
///
/// # Returns
/// The connected client on success, or the last error after max retries.
pub async fn connect_with_retry<T, E, F, Fut>(
    service_name: &str,
    address: &str,
    connect: F,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    const MAX_RETRIES: u32 = 30;
    const INITIAL_DELAY: Duration = Duration::from_millis(100);
    const MAX_DELAY: Duration = Duration::from_secs(5);

    let mut delay = INITIAL_DELAY;
    let mut attempt = 0;

    loop {
        attempt += 1;
        match connect().await {
            Ok(client) => {
                tracing::info!("Connected to {} at {}", service_name, address);
                return Ok(client);
            }
            Err(e) if attempt < MAX_RETRIES => {
                warn!(
                    "Failed to connect to {} (attempt {}/{}): {}. Retrying in {:?}...",
                    service_name, attempt, MAX_RETRIES, e, delay
                );
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, MAX_DELAY);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to connect to {} after {} attempts: {}",
                    service_name,
                    MAX_RETRIES,
                    e
                );
                return Err(e);
            }
        }
    }
}
