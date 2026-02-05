//! Order status enum for type-safe status handling.

/// Status values for the Order aggregate.
///
/// Provides compile-time safety for status comparisons and transitions.
/// Converts to/from string for proto compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrderStatus {
    Pending,
    PaymentSubmitted,
    Completed,
    Cancelled,
}

impl OrderStatus {
    /// Get the string representation of this status.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::PaymentSubmitted => "payment_submitted",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Allow comparing OrderStatus with &str directly.
impl PartialEq<str> for OrderStatus {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// Allow comparing &str with OrderStatus directly.
impl PartialEq<OrderStatus> for str {
    fn eq(&self, other: &OrderStatus) -> bool {
        self == other.as_str()
    }
}

/// Allow comparing OrderStatus with String directly.
impl PartialEq<String> for OrderStatus {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

/// Allow comparing String with OrderStatus directly.
impl PartialEq<OrderStatus> for String {
    fn eq(&self, other: &OrderStatus) -> bool {
        self.as_str() == other.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        assert_eq!(OrderStatus::Pending.as_str(), "pending");
        assert_eq!(OrderStatus::PaymentSubmitted.as_str(), "payment_submitted");
        assert_eq!(OrderStatus::Completed.as_str(), "completed");
        assert_eq!(OrderStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", OrderStatus::Pending), "pending");
    }

    #[test]
    fn test_eq_str() {
        assert!(OrderStatus::Pending == *"pending");
        assert!(OrderStatus::Completed == *"completed");
        assert!(OrderStatus::Pending != *"completed");
    }

    #[test]
    fn test_eq_string() {
        let status = "pending".to_string();
        assert!(OrderStatus::Pending == status);
        assert!(status == OrderStatus::Pending);
    }
}
