//! gRPC utilities.

use std::time::Duration;

use backon::{BackoffBuilder, ExponentialBuilder};
use tonic::transport::Channel;
use tracing::warn;

/// Connect to a gRPC endpoint with retry and exponential backoff.
///
/// Creates a channel connected to the given address.
/// The address should be in the format "host:port".
///
/// Retries connection with exponential backoff and jitter on failure.
pub async fn connect_channel(address: &str) -> Result<Channel, String> {
    // Exponential backoff with jitter for connection retries
    let backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(100))
        .with_max_delay(Duration::from_secs(5))
        .with_max_times(10)
        .with_jitter()
        .build();

    let mut last_error: Option<String> = None;

    for (attempt, delay) in std::iter::once(Duration::ZERO).chain(backoff).enumerate() {
        if attempt > 0 {
            warn!(
                address = %address,
                attempt = attempt,
                backoff_ms = %delay.as_millis(),
                "gRPC connection failed, retrying after backoff"
            );
            tokio::time::sleep(delay).await;
        }

        match Channel::from_shared(format!("http://{}", address)) {
            Ok(endpoint) => match endpoint.connect().await {
                Ok(channel) => return Ok(channel),
                Err(e) => {
                    last_error = Some(format!("Connection failed: {}", e));
                }
            },
            Err(e) => {
                return Err(format!("Invalid URI: {}", e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Connection failed after max retries".to_string()))
}
