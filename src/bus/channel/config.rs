//! Channel bus configuration.

/// Channel capacity for broadcast.
pub const CHANNEL_CAPACITY: usize = 1024;

/// Configuration for channel event bus.
#[derive(Clone, Debug, Default)]
pub struct ChannelConfig {
    /// Domain filter for subscribers.
    /// - `None` or `Some("#")` matches all domains
    /// - `Some("orders")` matches only "orders" domain
    pub domain_filter: Option<String>,
}

impl ChannelConfig {
    /// Create config for publishing only.
    pub fn publisher() -> Self {
        Self {
            domain_filter: None,
        }
    }

    /// Create config for subscribing to a specific domain.
    pub fn subscriber(domain: impl Into<String>) -> Self {
        Self {
            domain_filter: Some(domain.into()),
        }
    }

    /// Create config for subscribing to all domains.
    pub fn subscriber_all() -> Self {
        Self {
            domain_filter: Some("#".to_string()),
        }
    }
}

/// Check if a domain matches a filter pattern.
///
/// Matching rules:
/// - "#" matches all domains
/// - Exact match: "orders" matches "orders"
/// - Hierarchical: "orders" matches "orders.items" (prefix match with dot separator)
pub fn domain_matches(domain: &str, filter: &str) -> bool {
    if filter == "#" {
        return true;
    }
    if domain == filter {
        return true;
    }
    // Hierarchical match: filter is prefix of domain with dot separator
    domain.starts_with(filter) && domain[filter.len()..].starts_with('.')
}
