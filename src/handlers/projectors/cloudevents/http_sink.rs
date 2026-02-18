//! HTTP webhook sink for CloudEvents.
//!
//! POSTs CloudEvents batches to a configured endpoint using
//! `application/cloudevents-batch+json` content type.

use std::time::Duration;

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use reqwest::Client;
use tracing::{debug, error, warn};

use super::sink::{CloudEventsSink, SinkError};
use super::types::CloudEventEnvelope;

/// HTTP sink configuration.
#[derive(Debug, Clone)]
pub struct HttpSinkConfig {
    /// Webhook endpoint URL.
    pub endpoint: String,

    /// Request timeout.
    pub timeout: Duration,

    /// Maximum batch size (events per request).
    pub batch_size: usize,

    /// Additional headers to include.
    pub headers: Vec<(String, String)>,
}

impl Default for HttpSinkConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            timeout: Duration::from_secs(30),
            batch_size: 100,
            headers: Vec::new(),
        }
    }
}

impl HttpSinkConfig {
    /// Create config from environment variables.
    ///
    /// - `CLOUDEVENTS_HTTP_ENDPOINT`: Required webhook URL
    /// - `CLOUDEVENTS_HTTP_TIMEOUT`: Optional timeout in seconds (default: 30)
    /// - `CLOUDEVENTS_BATCH_SIZE`: Optional batch size (default: 100)
    pub fn from_env() -> Result<Self, SinkError> {
        let endpoint = std::env::var("CLOUDEVENTS_HTTP_ENDPOINT")
            .map_err(|_| SinkError::Config("CLOUDEVENTS_HTTP_ENDPOINT not set".to_string()))?;

        let timeout_secs = std::env::var("CLOUDEVENTS_HTTP_TIMEOUT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        let batch_size = std::env::var("CLOUDEVENTS_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        Ok(Self {
            endpoint,
            timeout: Duration::from_secs(timeout_secs),
            batch_size,
            headers: Vec::new(),
        })
    }

    /// Set the endpoint URL.
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = endpoint;
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the batch size.
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Add a header.
    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.push((key, value));
        self
    }
}

/// HTTP webhook sink.
///
/// Publishes CloudEvents batches to a webhook endpoint with retry.
pub struct HttpSink {
    client: Client,
    config: HttpSinkConfig,
}

impl HttpSink {
    /// Create a new HTTP sink with the given configuration.
    pub fn new(config: HttpSinkConfig) -> Result<Self, SinkError> {
        if config.endpoint.is_empty() {
            return Err(SinkError::Config(
                "HTTP endpoint not configured".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(SinkError::Http)?;

        Ok(Self { client, config })
    }

    /// Create from environment variables.
    pub fn from_env() -> Result<Self, SinkError> {
        let config = HttpSinkConfig::from_env()?;
        Self::new(config)
    }

    /// Backoff configuration for retries.
    fn backoff() -> ExponentialBuilder {
        ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(5))
            .with_max_times(5)
            .with_jitter()
    }

    /// Determine if an HTTP error is retryable.
    fn is_retryable(err: &reqwest::Error) -> bool {
        // Retry timeouts and connection errors
        err.is_timeout() || err.is_connect()
    }

    /// Determine if an HTTP status code is retryable.
    fn is_retryable_status(status: reqwest::StatusCode) -> bool {
        // Retry 429 (rate limit) and 5xx (server errors)
        status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
    }

    /// Post a batch of events to the webhook.
    async fn post_batch(&self, events: &[CloudEventEnvelope]) -> Result<(), SinkError> {
        let json = serde_json::to_string(&events)?;

        let mut request = self
            .client
            .post(&self.config.endpoint)
            .header("Content-Type", "application/cloudevents-batch+json")
            .body(json);

        // Add custom headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;

        let status = response.status();

        if status.is_success() {
            debug!(
                endpoint = %self.config.endpoint,
                event_count = events.len(),
                "CloudEvents batch posted successfully"
            );
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            let is_retryable = Self::is_retryable_status(status);

            if is_retryable {
                warn!(
                    endpoint = %self.config.endpoint,
                    status = %status,
                    body = %body,
                    "CloudEvents POST returned retryable status"
                );
            } else {
                error!(
                    endpoint = %self.config.endpoint,
                    status = %status,
                    body = %body,
                    "CloudEvents POST failed"
                );
            }

            // Use Unavailable for retryable, Config for non-retryable
            if is_retryable {
                Err(SinkError::Unavailable(format!(
                    "HTTP {} - {}",
                    status,
                    body.chars().take(200).collect::<String>()
                )))
            } else {
                Err(SinkError::Config(format!(
                    "HTTP {} - {}",
                    status,
                    body.chars().take(200).collect::<String>()
                )))
            }
        }
    }
}

#[async_trait]
impl CloudEventsSink for HttpSink {
    async fn publish(&self, events: Vec<CloudEventEnvelope>) -> Result<(), SinkError> {
        if events.is_empty() {
            return Ok(());
        }

        // Split into batches
        for batch in events.chunks(self.config.batch_size) {
            let batch_vec: Vec<_> = batch.to_vec();

            // Retry with backoff on transient failures
            let result = (|| async { self.post_batch(&batch_vec).await })
                .retry(Self::backoff())
                .when(|e| {
                    matches!(
                        e,
                        SinkError::Http(err) if Self::is_retryable(err)
                    ) || matches!(e, SinkError::Unavailable(_))
                })
                .await;

            result?;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "http"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = HttpSinkConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.batch_size, 100);
        assert!(config.headers.is_empty());
    }

    #[test]
    fn test_config_builder() {
        let config = HttpSinkConfig::default()
            .with_endpoint("https://example.com/events".to_string())
            .with_timeout(Duration::from_secs(60))
            .with_batch_size(50)
            .with_header("Authorization".to_string(), "Bearer token".to_string());

        assert_eq!(config.endpoint, "https://example.com/events");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.headers.len(), 1);
    }

    #[test]
    fn test_empty_endpoint_fails() {
        let config = HttpSinkConfig::default();
        let result = HttpSink::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_retryable_status_codes() {
        use reqwest::StatusCode;

        assert!(HttpSink::is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(HttpSink::is_retryable_status(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(HttpSink::is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(HttpSink::is_retryable_status(
            StatusCode::SERVICE_UNAVAILABLE
        ));

        assert!(!HttpSink::is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!HttpSink::is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!HttpSink::is_retryable_status(StatusCode::NOT_FOUND));
    }
}
