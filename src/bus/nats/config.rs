//! NATS JetStream configuration.

/// Default subject prefix for NATS streams.
pub const DEFAULT_PREFIX: &str = "angzarr";

/// Default edition name.
pub const DEFAULT_EDITION: &str = "angzarr";

/// Header name for angzarr correlation ID.
pub const HEADER_CORRELATION: &str = "Angzarr-Correlation";

/// Configuration for NATS EventBus.
#[derive(Debug, Clone)]
pub struct NatsBusConfig {
    /// Subject prefix (default: "angzarr")
    pub prefix: String,
    /// Consumer/subscriber name
    pub consumer_name: Option<String>,
    /// Domain filter (None = all domains)
    pub domain_filter: Option<String>,
}

impl Default for NatsBusConfig {
    fn default() -> Self {
        Self {
            prefix: DEFAULT_PREFIX.to_string(),
            consumer_name: None,
            domain_filter: None,
        }
    }
}
