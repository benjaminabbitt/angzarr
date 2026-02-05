//! Fulfillment status enum for type-safe status handling.

/// Status values for the Fulfillment aggregate.
///
/// Represents the shipment lifecycle: pending -> picking -> packing -> shipped -> delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FulfillmentStatus {
    Pending,
    Picking,
    Packing,
    Shipped,
    Delivered,
}

impl FulfillmentStatus {
    /// Get the string representation of this status.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Picking => "picking",
            Self::Packing => "packing",
            Self::Shipped => "shipped",
            Self::Delivered => "delivered",
        }
    }
}

impl std::fmt::Display for FulfillmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Allow comparing FulfillmentStatus with &str directly.
impl PartialEq<str> for FulfillmentStatus {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// Allow comparing &str with FulfillmentStatus directly.
impl PartialEq<FulfillmentStatus> for str {
    fn eq(&self, other: &FulfillmentStatus) -> bool {
        self == other.as_str()
    }
}

/// Allow comparing FulfillmentStatus with String directly.
impl PartialEq<String> for FulfillmentStatus {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

/// Allow comparing String with FulfillmentStatus directly.
impl PartialEq<FulfillmentStatus> for String {
    fn eq(&self, other: &FulfillmentStatus) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(FulfillmentStatus::Pending.as_str(), "pending");
        assert_eq!(FulfillmentStatus::Picking.as_str(), "picking");
        assert_eq!(FulfillmentStatus::Packing.as_str(), "packing");
        assert_eq!(FulfillmentStatus::Shipped.as_str(), "shipped");
        assert_eq!(FulfillmentStatus::Delivered.as_str(), "delivered");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", FulfillmentStatus::Pending), "pending");
    }

    #[test]
    fn test_eq_str() {
        assert!(FulfillmentStatus::Pending == *"pending");
        assert!(FulfillmentStatus::Shipped == *"shipped");
        assert!(FulfillmentStatus::Pending != *"shipped");
    }

    #[test]
    fn test_eq_string() {
        let status = "pending".to_string();
        assert!(FulfillmentStatus::Pending == status);
        assert!(status == FulfillmentStatus::Pending);
    }
}
