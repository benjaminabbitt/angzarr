//! Aggregate identity computation for Angzarr domains.
//!
//! Provides deterministic UUID generation from business keys, ensuring consistent
//! aggregate identification across services.

use uuid::Uuid;

/// Compute a deterministic root UUID from domain and business key.
///
/// The UUID is derived from: `hash("angzarr" + domain + business_key)`
///
/// This ensures:
/// - Same business key always produces same root
/// - Different domains with same key produce different roots
/// - Collisions at business key level are handled by Angzarr
pub fn compute_root(domain: &str, business_key: &str) -> Uuid {
    let seed = format!("angzarr{}{}", domain, business_key);
    Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes())
}

/// Compute root UUID for a customer aggregate.
///
/// Business key: email address
pub fn customer_root(email: &str) -> Uuid {
    compute_root("customer", email)
}

/// Compute root UUID for a product aggregate.
///
/// Business key: SKU
pub fn product_root(sku: &str) -> Uuid {
    compute_root("product", sku)
}

/// Compute root UUID for an order aggregate.
///
/// Business key: order ID (typically a generated identifier)
pub fn order_root(order_id: &str) -> Uuid {
    compute_root("order", order_id)
}

/// Compute root UUID for an inventory aggregate.
///
/// Business key: product ID
pub fn inventory_root(product_id: &str) -> Uuid {
    compute_root("inventory", product_id)
}

/// Compute root UUID for a cart aggregate.
///
/// Business key: customer ID (one cart per customer)
pub fn cart_root(customer_id: &str) -> Uuid {
    compute_root("cart", customer_id)
}

/// Compute root UUID for a fulfillment aggregate.
///
/// Business key: order ID
pub fn fulfillment_root(order_id: &str) -> Uuid {
    compute_root("fulfillment", order_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_root_deterministic() {
        let root1 = compute_root("customer", "alice@example.com");
        let root2 = compute_root("customer", "alice@example.com");
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_compute_root_different_keys() {
        let root1 = compute_root("customer", "alice@example.com");
        let root2 = compute_root("customer", "bob@example.com");
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_compute_root_different_domains() {
        let root1 = compute_root("customer", "test-123");
        let root2 = compute_root("order", "test-123");
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_customer_root() {
        let root = customer_root("alice@example.com");
        assert_eq!(root, compute_root("customer", "alice@example.com"));
    }
}
