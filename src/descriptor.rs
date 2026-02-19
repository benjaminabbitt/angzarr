//! Subscription target types for event filtering.
//!
//! Used internally for subscription matching in sagas and process managers.

/// A subscription target: domain + event types to filter.
///
/// Used to declare which events a component subscribes to.
///
/// # Example
///
/// ```
/// use angzarr::descriptor::Target;
///
/// // Subscribe to all events from "order" domain
/// let all_orders = Target::domain("order");
///
/// // Subscribe to specific event types
/// let specific = Target::new("order", vec!["OrderCreated", "OrderShipped"]);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// Domain to subscribe to.
    pub domain: String,
    /// Event types to filter. Empty means all events from domain.
    pub types: Vec<String>,
}

impl Target {
    /// Create a target for a domain with specific event types.
    pub fn new(domain: impl Into<String>, types: Vec<impl Into<String>>) -> Self {
        Self {
            domain: domain.into(),
            types: types.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a target for all events from a domain.
    pub fn domain(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            types: Vec::new(),
        }
    }

    /// Check if an event type matches this target.
    ///
    /// Returns true if:
    /// - types is empty (matches all), OR
    /// - event_type ends with any type in the list
    pub fn matches_type(&self, event_type: &str) -> bool {
        if self.types.is_empty() {
            return true;
        }
        self.types.iter().any(|t| event_type.ends_with(t))
    }
}

/// Parse subscriptions from environment variable.
///
/// Format: `domain1:Type1,Type2;domain2:Type3` or `domain1;domain2` (all types).
///
/// # Example
///
/// ```
/// use angzarr::descriptor::parse_subscriptions;
///
/// // Parse specific types
/// let subs = parse_subscriptions("order:OrderCreated,OrderShipped;inventory:StockReserved");
/// assert_eq!(subs.len(), 2);
/// assert_eq!(subs[0].domain, "order");
/// assert_eq!(subs[0].types, vec!["OrderCreated", "OrderShipped"]);
///
/// // Parse all events from domain
/// let subs = parse_subscriptions("order;inventory");
/// assert_eq!(subs.len(), 2);
/// assert!(subs[0].types.is_empty());
/// ```
pub fn parse_subscriptions(env_value: &str) -> Vec<Target> {
    if env_value.is_empty() {
        return Vec::new();
    }

    env_value
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|part| {
            if let Some((domain, types_str)) = part.split_once(':') {
                let types: Vec<String> = types_str
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect();
                Target::new(domain, types)
            } else {
                Target::domain(part)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_matches_all() {
        let target = Target::domain("order");
        assert!(target.matches_type("OrderCreated"));
        assert!(target.matches_type("OrderShipped"));
        assert!(target.matches_type("anything"));
    }

    #[test]
    fn test_target_matches_specific() {
        let target = Target::new("order", vec!["OrderCreated", "OrderShipped"]);
        assert!(target.matches_type("type.googleapis.com/examples.OrderCreated"));
        assert!(target.matches_type("OrderShipped"));
        assert!(!target.matches_type("OrderCancelled"));
    }

    #[test]
    fn test_parse_subscriptions_with_types() {
        let subs = parse_subscriptions("order:OrderCreated,OrderShipped;inventory:StockReserved");
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].domain, "order");
        assert_eq!(subs[0].types, vec!["OrderCreated", "OrderShipped"]);
        assert_eq!(subs[1].domain, "inventory");
        assert_eq!(subs[1].types, vec!["StockReserved"]);
    }

    #[test]
    fn test_parse_subscriptions_all_types() {
        let subs = parse_subscriptions("order;inventory");
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].domain, "order");
        assert!(subs[0].types.is_empty());
        assert_eq!(subs[1].domain, "inventory");
        assert!(subs[1].types.is_empty());
    }

    #[test]
    fn test_parse_subscriptions_empty() {
        let subs = parse_subscriptions("");
        assert!(subs.is_empty());
    }

    #[test]
    fn test_parse_subscriptions_mixed() {
        let subs = parse_subscriptions("order:OrderCreated;inventory");
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].types, vec!["OrderCreated"]);
        assert!(subs[1].types.is_empty());
    }
}
